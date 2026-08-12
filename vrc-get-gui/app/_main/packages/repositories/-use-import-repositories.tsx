import { queryOptions } from "@tanstack/react-query";
import { CircleAlert, RefreshCw } from "lucide-react";
import type React from "react";
import { useEffect, useRef, useState } from "react";
import {
	Accordion,
	AccordionContent,
	AccordionItem,
	AccordionTrigger,
} from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import {
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { assertNever } from "@/lib/assert-never";
import type {
	TauriDownloadRepository,
	TauriRepositoryDescriptor,
} from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { callAsyncCommand } from "@/lib/call-async-command";
import { type DialogContext, showDialog } from "@/lib/dialog";
import { tc, tt } from "@/lib/i18n";
import { countProcessedSteps } from "@/lib/operation-progress";
import { queryClient } from "@/lib/query-client";
import { toastSuccess, toastThrownError } from "@/lib/toast";
import { useEffectEvent } from "@/lib/use-effect-event";
import { RepositoryPackageList } from "./-repository-package-list";

type ParsedRepositories = {
	repositories: TauriRepositoryDescriptor[];
	unparsable_lines: string[];
};

const environmentRepositoriesInfo = queryOptions({
	queryKey: ["environmentRepositoriesInfo"],
	queryFn: commands.environmentRepositoriesInfo,
});

const environmentPackages = queryOptions({
	queryKey: ["environmentPackages"],
	queryFn: commands.environmentPackages,
});

const environmentRepositoryPackageLists = queryOptions({
	queryKey: ["environmentRepositoryPackageLists"],
	queryFn: commands.environmentRepositoryPackageLists,
});

let repositoryImportInProgress = false;

export async function importRepositories() {
	if (repositoryImportInProgress) return;
	repositoryImportInProgress = true;
	try {
		await importRepositoriesImpl();
	} finally {
		repositoryImportInProgress = false;
	}
}

async function importRepositoriesImpl() {
	using dialog = showDialog(
		null,
		"large-dialog-content overflow-hidden",
		true,
		tc("vpm repositories:import progress:restore"),
	);

	const pickResult = await commands.environmentImportRepositoryPick();
	switch (pickResult.type) {
		case "NoFilePicked":
			// no-op
			return;
		case "ParsedRepositories":
			// continue
			break;
		default:
			assertNever(pickResult, "pickResult");
	}
	console.log("confirmingRepositories", pickResult);

	const repositories = await dialog.ask(ConfirmingRepositoryList, {
		pickResult,
	});
	if (repositories == null) return;

	const initialPackages = await dialog.ask(LoadingRepositories, {
		repositories,
	});
	if (initialPackages == null) return;
	let packages: [TauriRepositoryDescriptor, TauriDownloadRepository][] =
		initialPackages;

	let repositoriesToAdd: PreparedRepositoryImport[];
	while (true) {
		const confirmation = await dialog.ask(ConfirmingPackages, { packages });
		if (confirmation == null) {
			await discardDownloadedRepositories(packages);
			return;
		}
		if (confirmation.type === "retry") {
			const retried = await dialog.ask(LoadingRepositories, {
				repositories: confirmation.indices.map((index) => packages[index][0]),
			});
			if (retried == null) {
				await discardDownloadedRepositories(packages);
				return;
			}
			const nextPackages = [...packages];
			confirmation.indices.forEach((index, retryIndex) => {
				nextPackages[index] = retried[retryIndex];
			});
			packages = nextPackages;
			continue;
		}
		repositoriesToAdd = confirmation.repositories;
		break;
	}
	if (repositoriesToAdd.length === 0) {
		dialog.close();
		return;
	}

	dialog.setEscapeBehavior(false);
	await dialog.askClosing(SavingRepositories, {
		repositories: repositoriesToAdd,
	});
}

function shortRepositoryDescription(
	repo: TauriRepositoryDescriptor,
): React.ReactNode {
	if (Object.keys(repo.headers).length > 0) {
		return tc("vpm repositories:dialog:repository with headers", {
			repoUrl: repo.url,
		});
	}
	return repo.url;
}

function ConfirmingRepositoryList({
	pickResult,
	dialog,
}: {
	pickResult: ParsedRepositories;
	dialog: DialogContext<TauriRepositoryDescriptor[] | null>;
}) {
	return (
		<>
			<ScrollArea
				type="scroll"
				className="max-h-[min(560px,calc(100dvh-12rem))] w-full font-normal"
				scrollBarClassName="bg-transparent py-2.5"
			>
				<div className="pr-4">
					<p className={"font-normal whitespace-normal"}>
						{tc("vpm repositories:dialog:confirm repository list")}
					</p>

					<ul className={"list-disc pl-6"}>
						{pickResult.repositories.map((info) => (
							<li key={info.url}>{shortRepositoryDescription(info)}</li>
						))}
					</ul>

					{pickResult.unparsable_lines.length > 0 && (
						<>
							<p className={"font-normal whitespace-normal"}>
								{tc("vpm repositories:dialog:unparsable lines list")}
							</p>
							<ul className={"list-disc pl-6"}>
								{pickResult.unparsable_lines.map((line, idx) => (
									// biome-ignore lint/suspicious/noArrayIndexKey: unchanged
									<li key={idx} className={"whitespace-pre"}>
										{line}
									</li>
								))}
							</ul>
						</>
					)}
				</div>
			</ScrollArea>
			<DialogFooter className={"gap-2"}>
				<Button onClick={() => dialog.close(null)}>
					{tc("general:button:cancel")}
				</Button>
				<Button onClick={() => dialog.close(pickResult.repositories)}>
					{tc("vpm repositories:dialog:button:continue importing repositories")}
				</Button>
			</DialogFooter>
		</>
	);
}

function LoadingRepositories({
	repositories,
	dialog,
}: {
	repositories: TauriRepositoryDescriptor[];
	dialog: DialogContext<
		[TauriRepositoryDescriptor, TauriDownloadRepository][] | null
	>;
}) {
	const cancelRef = useRef<() => void>(() => {});
	const [items, setItems] = useState<RepositoryImportItem[]>(() =>
		repositories.map((repository) => ({
			key: crypto.randomUUID(),
			repository,
			status: "waiting",
			downloadFinished: false,
		})),
	);

	const event = useEffectEvent(() => {
		const [cancel, resultPromise] = callAsyncCommand(
			commands.environmentImportDownloadRepositories,
			[repositories],
			(progress) =>
				setItems((current) =>
					current.map((item, index) => {
						if (index !== progress.index) return item;
						switch (progress.type) {
							case "DownloadStarted":
								return { ...item, status: "downloading" };
							case "DownloadFinished":
								return {
									...item,
									status: "downloaded",
									downloadFinished: true,
								};
							case "Failed":
								return {
									...item,
									status: "failed",
									message: progress.message,
								};
							default:
								return assertNever(progress);
						}
					}),
				),
		);
		cancelRef.current = cancel;
		resultPromise.then(
			(x) => {
				cancelRef.current = () => {};
				dialog.close(x === "cancelled" ? null : x);
			},
			(error) => {
				cancelRef.current = () => {};
				dialog.error(error);
			},
		);
	});

	useEffect(() => {
		event();
		return () => cancelRef.current();
	}, []);
	const processed = countProcessedSteps(
		items,
		1,
		(item) => (item.downloadFinished ? 1 : 0),
		(item) => item.status === "failed",
	);

	return (
		<>
			<DialogHeader>
				<DialogTitle className="flex items-center gap-2">
					<RefreshCw className="size-5 animate-spin" />
					{tc("vpm repositories:import progress:title")}
				</DialogTitle>
				<DialogDescription>
					{tc("vpm repositories:import progress:description")}
				</DialogDescription>
			</DialogHeader>
			<div className="space-y-2">
				<p>{tc("vpm repositories:dialog:downloading repositories...")}</p>
				<Progress value={processed} max={items.length} />
				<div className={"text-center"}>
					{tc("vpm repositories:dialog:processed n/m", {
						processed,
						totalCount: items.length,
					})}
				</div>
			</div>
			<RepositoryImportItems items={items} />
			<DialogFooter>
				<Button onClick={() => cancelRef.current?.()}>
					{tc("general:button:cancel")}
				</Button>
			</DialogFooter>
		</>
	);
}

function ConfirmingPackages({
	packages,
	dialog,
}: {
	packages: [TauriRepositoryDescriptor, TauriDownloadRepository][];
	dialog: DialogContext<ConfirmingPackagesResult>;
}) {
	const failedIndices = packages.flatMap(([_, download], index) =>
		download.type === "DownloadError" ? [index] : [],
	);

	function add() {
		dialog.close({
			type: "add",
			repositories: packages.flatMap(([repository, download]) =>
				download.type === "Success"
					? [{ repository, downloadId: download.download_id }]
					: [],
			),
		});
	}

	return (
		<>
			<div className={"flex min-h-0 flex-col font-normal"}>
				<p className={"whitespace-normal"}>
					{tc("vpm repositories:dialog:confirm packages list")}
				</p>
				<ScrollArea
					type="scroll"
					className="h-[min(560px,calc(100dvh-14rem))] w-full"
					scrollBarClassName="bg-transparent py-2.5"
				>
					<div className="pr-4">
						<Accordion type="single" collapsible className="w-full">
							{packages.map(([repo, download]) => {
								let toneClass = "";
								let content: React.ReactNode;
								switch (download.type) {
									case "BadUrl":
										throw new Error("BadUrl should not be here");
									case "Duplicated":
										toneClass = "text-warning";
										content = (
											<>
												<p>
													{tc("vpm repositories:dialog:name", {
														name: download.duplicated_name,
													})}
												</p>
												{download.duplicated_original_name != null && (
													<p className="text-muted-foreground text-sm">
														{tc("vpm repositories:original name", {
															name: download.duplicated_original_name,
														})}
													</p>
												)}
											</>
										);
										break;
									case "DownloadError":
										toneClass = "text-destructive";
										content = tc(
											"vpm repositories:dialog:download error:download error",
										);
										break;
									case "Success":
										content = (
											<RepositoryPackageList
												packages={download.value.packages}
											/>
										);
										break;
									default:
										assertNever(download, "download");
								}
								return (
									<AccordionItem value={repo.url} key={repo.url}>
										<AccordionTrigger className={`${toneClass} py-2 text-base`}>
											{shortRepositoryDescription(repo)}
										</AccordionTrigger>
										<AccordionContent className={toneClass}>
											{content}
										</AccordionContent>
									</AccordionItem>
								);
							})}
						</Accordion>
					</div>
				</ScrollArea>
			</div>
			<DialogFooter>
				<Button onClick={() => dialog.close(null)}>
					{tc("general:button:cancel")}
				</Button>
				{failedIndices.length > 0 && (
					<Button
						className="gap-2"
						onClick={() =>
							dialog.close({ type: "retry", indices: failedIndices })
						}
					>
						<RefreshCw className="size-4" />
						{tc("vpm repositories:import progress:retry")}
					</Button>
				)}
				<Button onClick={add} className={"ml-2"}>
					{tc("vpm repositories:button:add repositories")}
				</Button>
			</DialogFooter>
		</>
	);
}

type RepositoryImportItemStatus =
	| "waiting"
	| "downloading"
	| "downloaded"
	| "completed"
	| "failed";

type RepositoryImportStatus = "saving" | "partial" | "failed";

type PreparedRepositoryImport = {
	repository: TauriRepositoryDescriptor;
	downloadId: string;
};

type ConfirmingPackagesResult =
	| { type: "add"; repositories: PreparedRepositoryImport[] }
	| { type: "retry"; indices: number[] }
	| null;

type RepositoryImportItem = {
	key: string;
	repository: TauriRepositoryDescriptor;
	status: RepositoryImportItemStatus;
	downloadFinished: boolean;
	downloadId?: string;
	message?: string;
};

type RepositoryImportTarget = {
	itemIndex: number;
	downloadId: string;
};

function SavingRepositories({
	repositories,
	dialog,
}: {
	repositories: PreparedRepositoryImport[];
	dialog: DialogContext<void>;
}) {
	const [items, setItems] = useState<RepositoryImportItem[]>(() =>
		repositories.map(({ repository, downloadId }) => ({
			key: crypto.randomUUID(),
			repository,
			downloadId,
			status: "downloaded",
			downloadFinished: true,
		})),
	);
	const [status, setStatus] = useState<RepositoryImportStatus>("saving");
	const startedRef = useRef(false);

	const runAttempt = useEffectEvent(
		async (targets: RepositoryImportTarget[]) => {
			const previouslyCompleted = items.filter(
				(item) => item.status === "completed",
			).length;
			const targetIndices = new Set(targets.map((target) => target.itemIndex));
			setItems((current) =>
				current.map((item, index) =>
					targetIndices.has(index)
						? {
								...item,
								status: "downloaded",
								downloadFinished: true,
								message: undefined,
							}
						: item,
				),
			);
			setStatus("saving");

			try {
				const result = await commands.environmentImportAddRepositories(
					targets.map((target) => target.downloadId),
				);
				const succeeded = new Set(result.succeeded);
				const failed = new Map(
					result.failed.map((failure) => [failure.index, failure.message]),
				);
				setItems((current) =>
					current.map((item, index) => {
						const progressIndex = targets.findIndex(
							(target) => target.itemIndex === index,
						);
						if (succeeded.has(progressIndex)) {
							return {
								...item,
								status: "completed",
								downloadFinished: true,
							};
						}
						const failureMessage = failed.get(progressIndex);
						if (failureMessage != null) {
							return {
								...item,
								status: "failed",
								downloadFinished: true,
								message: failureMessage,
							};
						}
						return item;
					}),
				);

				if (result.failed.length === 0) {
					toastSuccess(tt("vpm repositories:toast:repository added"));
					dialog.close();
					void refreshImportedRepositories();
					return;
				} else if (previouslyCompleted + result.succeeded.length > 0) {
					setStatus("partial");
				} else {
					setStatus("failed");
				}
				if (result.succeeded.length > 0) {
					void refreshImportedRepositories();
				}
			} catch (error) {
				console.error(error);
				toastThrownError(error);
				setItems((current) =>
					current.map((item, index) =>
						targetIndices.has(index) && item.status !== "completed"
							? { ...item, status: "failed" }
							: item,
					),
				);
				setStatus(previouslyCompleted > 0 ? "partial" : "failed");
			}
		},
	);

	useEffect(() => {
		if (startedRef.current) return;
		startedRef.current = true;
		void runAttempt(
			repositories.map(({ downloadId }, itemIndex) => ({
				itemIndex,
				downloadId,
			})),
		);
	}, [repositories]);

	const retryTargets = items
		.map((item, itemIndex) => ({ item, itemIndex }))
		.filter(({ item }) => item.status === "failed" && item.downloadId != null)
		.map(({ item, itemIndex }) => ({
			itemIndex,
			downloadId: item.downloadId as string,
		}));
	const completedCount = items.filter(
		(item) => item.status === "completed",
	).length;
	const failedCount = items.filter((item) => item.status === "failed").length;
	const active = status === "saving";

	async function close() {
		try {
			await commands.environmentDiscardRepositoryDownloads(
				retryTargets.map((target) => target.downloadId),
			);
		} finally {
			dialog.close();
		}
	}

	if (active) {
		return (
			<div className="flex items-center gap-2">
				<RefreshCw className="size-5 animate-spin" />
				<p>{tc("vpm repositories:dialog:adding repositories...")}</p>
			</div>
		);
	}

	return (
		<>
			<DialogHeader>
				<DialogTitle className="flex items-center gap-2">
					<CircleAlert className="size-5 text-destructive" />
					{repositoryImportTitle(status)}
				</DialogTitle>
				<DialogDescription>
					{tc("vpm repositories:import progress:description")}
				</DialogDescription>
			</DialogHeader>
			<div className="space-y-2">
				<p className="text-center text-sm text-muted-foreground">
					{tc("vpm repositories:import progress:summary", {
						completed: completedCount,
						failed: failedCount,
					})}
				</p>
			</div>
			<RepositoryImportItems items={items} />
			<DialogFooter>
				{!active && (
					<>
						{retryTargets.length > 0 && (
							<Button
								className="gap-2"
								onClick={() => void runAttempt(retryTargets)}
							>
								<RefreshCw className="size-4" />
								{tc("vpm repositories:import progress:retry")}
							</Button>
						)}
						<Button onClick={() => void close()}>
							{tc("general:button:close")}
						</Button>
					</>
				)}
			</DialogFooter>
		</>
	);
}

async function refreshImportedRepositories() {
	try {
		await Promise.all([
			queryClient.invalidateQueries(environmentRepositoriesInfo),
			queryClient.invalidateQueries(environmentPackages),
			queryClient.invalidateQueries(environmentRepositoryPackageLists),
		]);
		await commands.environmentRefetchPackages();
		await Promise.all([
			queryClient.invalidateQueries(environmentRepositoriesInfo),
			queryClient.invalidateQueries(environmentPackages),
			queryClient.invalidateQueries(environmentRepositoryPackageLists),
		]);
	} catch (error) {
		console.error(error);
		toastThrownError(error);
	}
}

function repositoryImportTitle(status: RepositoryImportStatus) {
	switch (status) {
		case "saving":
			return tc("vpm repositories:import progress:finalizing");
		case "partial":
			return tc("vpm repositories:import progress:partial");
		case "failed":
			return tc("vpm repositories:import progress:failed");
		default:
			return assertNever(status);
	}
}

function repositoryImportStatusLabel(status: RepositoryImportItemStatus) {
	switch (status) {
		case "waiting":
			return tc("vpm repositories:import progress:status:waiting");
		case "downloading":
			return tc("vpm repositories:import progress:status:downloading");
		case "downloaded":
			return tc("vpm repositories:import progress:status:downloaded");
		case "completed":
			return tc("vpm repositories:import progress:status:completed");
		case "failed":
			return tc("vpm repositories:import progress:status:failed");
		default:
			return assertNever(status);
	}
}

function repositoryImportStatusClass(status: RepositoryImportItemStatus) {
	switch (status) {
		case "completed":
			return "text-success";
		case "failed":
			return "text-destructive";
		default:
			return "text-muted-foreground";
	}
}

function RepositoryImportItems({ items }: { items: RepositoryImportItem[] }) {
	return (
		<div className="overflow-hidden rounded-[1rem] bg-secondary/40">
			<ScrollArea className="h-[min(420px,40vh)]">
				<div className="p-2">
					{items.map((item) => (
						<div
							className="flex items-center justify-between gap-3 p-2"
							key={item.key}
						>
							<p className="min-w-0 truncate font-normal">
								{shortRepositoryDescription(item.repository)}
							</p>
							<p
								className={`shrink-0 text-sm ${repositoryImportStatusClass(item.status)}`}
								title={item.message}
							>
								{repositoryImportStatusLabel(item.status)}
							</p>
						</div>
					))}
				</div>
			</ScrollArea>
		</div>
	);
}

async function discardDownloadedRepositories(
	packages: [TauriRepositoryDescriptor, TauriDownloadRepository][],
) {
	const downloadIds = packages.flatMap(([_, download]) =>
		download.type === "Success" ? [download.download_id] : [],
	);
	if (downloadIds.length === 0) return;
	await commands.environmentDiscardRepositoryDownloads(downloadIds);
}
