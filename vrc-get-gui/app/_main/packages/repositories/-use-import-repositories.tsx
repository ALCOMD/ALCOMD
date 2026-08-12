import { queryOptions } from "@tanstack/react-query";
import { CheckCircle2, CircleAlert, Minimize2, RefreshCw } from "lucide-react";
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
	TauriImportRepositoryProgress,
	TauriRepositoryDescriptor,
} from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { callAsyncCommand } from "@/lib/call-async-command";
import { type DialogContext, showDialog } from "@/lib/dialog";
import { tc, tt } from "@/lib/i18n";
import {
	countProcessedSteps,
	progressWithFinalStep,
} from "@/lib/operation-progress";
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

	const packages = await dialog.ask(LoadingRepositories, {
		repositories,
	});
	if (packages == null) return;

	const repositoriesToAdd = await dialog.ask(ConfirmingPackages, {
		packages,
	});
	if (repositoriesToAdd == null) return;
	if (repositoriesToAdd.length === 0) {
		dialog.close();
		return;
	}

	dialog.setEscapeBehavior("minimize");
	await dialog.askClosing(ImportingRepositories, {
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
	const totalCount = repositories.length;
	const [downloaded, setDownloaded] = useState(0);

	const event = useEffectEvent(() => {
		const [cancel, resultPromise] = callAsyncCommand(
			commands.environmentImportDownloadRepositories,
			[repositories],
			(downloaded) => setDownloaded(downloaded),
		);
		cancelRef.current = cancel;
		resultPromise.then(
			(x) => dialog.close(x === "cancelled" ? null : x),
			(error) => dialog.error(error),
		);
	});

	useEffect(() => event(), []);

	return (
		<>
			<div>
				<p>{tc("vpm repositories:dialog:downloading repositories...")}</p>
				<Progress value={downloaded} max={totalCount} />
				<div className={"text-center"}>
					{tc("vpm repositories:dialog:downloaded n/m", {
						downloaded,
						totalCount,
					})}
				</div>
			</div>
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
	dialog: DialogContext<TauriRepositoryDescriptor[] | null>;
}) {
	async function add() {
		dialog.close(
			packages
				.filter(([_, download]) => download.type === "Success")
				.map(([repo, _]) => repo),
		);
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
	| "failed"
	| "cancelled";

type RepositoryImportStatus =
	| "running"
	| "finalizing"
	| "completed"
	| "partial"
	| "failed"
	| "cancelled";

type RepositoryImportItem = {
	key: string;
	repository: TauriRepositoryDescriptor;
	status: RepositoryImportItemStatus;
	downloadFinished: boolean;
	message?: string;
};

type RepositoryImportTarget = {
	itemIndex: number;
	repository: TauriRepositoryDescriptor;
};

function ImportingRepositories({
	repositories,
	dialog,
}: {
	repositories: TauriRepositoryDescriptor[];
	dialog: DialogContext<void>;
}) {
	const [items, setItems] = useState<RepositoryImportItem[]>(() =>
		repositories.map((repository) => ({
			key: crypto.randomUUID(),
			repository,
			status: "waiting",
			downloadFinished: false,
		})),
	);
	const [status, setStatus] = useState<RepositoryImportStatus>("running");
	const [cancelRequested, setCancelRequested] = useState(false);
	const cancelRef = useRef<() => void>(() => {});
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
								status: "waiting",
								downloadFinished: false,
								message: undefined,
							}
						: item,
				),
			);
			setStatus("running");
			setCancelRequested(false);

			const updateItem = (
				progressIndex: number,
				update: (item: RepositoryImportItem) => RepositoryImportItem,
			) => {
				const itemIndex = targets[progressIndex]?.itemIndex;
				if (itemIndex == null) return;
				setItems((current) =>
					current.map((item, index) =>
						index === itemIndex ? update(item) : item,
					),
				);
			};

			const onProgress = (progress: TauriImportRepositoryProgress) => {
				switch (progress.type) {
					case "DownloadStarted":
						updateItem(progress.index, (item) => ({
							...item,
							status: "downloading",
						}));
						break;
					case "DownloadFinished":
						updateItem(progress.index, (item) => ({
							...item,
							status: "downloaded",
							downloadFinished: true,
						}));
						break;
					case "Finalizing":
						setStatus("finalizing");
						break;
					case "Failed":
						updateItem(progress.index, (item) => ({
							...item,
							status: "failed",
							downloadFinished: false,
							message: progress.message,
						}));
						break;
					default:
						assertNever(progress);
				}
			};

			const [cancel, resultPromise] = callAsyncCommand(
				commands.environmentImportAddRepositories,
				[targets.map((target) => target.repository)],
				onProgress,
			);
			cancelRef.current = cancel;

			try {
				const result = await resultPromise;
				if (result === "cancelled") {
					setItems((current) =>
						current.map((item, index) =>
							targetIndices.has(index) && item.status !== "completed"
								? {
										...item,
										status: "cancelled",
										downloadFinished: false,
									}
								: item,
						),
					);
					setStatus("cancelled");
					dialog.restore();
					return;
				}

				const succeeded = new Set(result.succeeded);
				const failed = new Set(result.failed);
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
						if (failed.has(progressIndex)) {
							return {
								...item,
								status: "failed",
								downloadFinished: false,
							};
						}
						return item;
					}),
				);

				if (result.failed.length === 0) {
					setStatus("completed");
					toastSuccess(tt("vpm repositories:toast:repository added"));
				} else if (previouslyCompleted + result.succeeded.length > 0) {
					setStatus("partial");
				} else {
					setStatus("failed");
				}
				dialog.restore();
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
				dialog.restore();
			} finally {
				cancelRef.current = () => {};
				setCancelRequested(false);
			}
		},
	);

	useEffect(() => {
		if (startedRef.current) return;
		startedRef.current = true;
		void runAttempt(
			repositories.map((repository, itemIndex) => ({
				itemIndex,
				repository,
			})),
		);
	}, [repositories]);

	const retryTargets = items
		.map((item, itemIndex) => ({ item, itemIndex }))
		.filter(
			({ item }) => item.status === "failed" || item.status === "cancelled",
		)
		.map(({ item, itemIndex }) => ({
			itemIndex,
			repository: item.repository,
		}));
	const completedCount = items.filter(
		(item) => item.status === "completed",
	).length;
	const failedCount = items.filter((item) => item.status === "failed").length;
	const completedSteps = countProcessedSteps(
		items,
		1,
		(item) => (item.downloadFinished ? 1 : 0),
		(item) => item.status === "failed",
	);
	const progress = progressWithFinalStep(
		completedSteps,
		items.length,
		status === "completed" || status === "partial" || status === "failed",
	);
	const active = status === "running" || status === "finalizing";
	const canCancel = status === "running" && !cancelRequested;

	return (
		<>
			<DialogHeader>
				<DialogTitle className="flex items-center gap-2">
					{status === "completed" ? (
						<CheckCircle2 className="size-5 text-success" />
					) : status === "partial" || status === "failed" ? (
						<CircleAlert className="size-5 text-destructive" />
					) : status === "cancelled" ? (
						<CircleAlert className="size-5 text-warning" />
					) : (
						<RefreshCw className="size-5 animate-spin" />
					)}
					{repositoryImportTitle(status)}
				</DialogTitle>
				<DialogDescription>
					{tc("vpm repositories:import progress:description")}
				</DialogDescription>
			</DialogHeader>
			<div className="space-y-2">
				<Progress value={progress} max={100} />
				<p className="text-center text-sm text-muted-foreground">
					{tc("vpm repositories:import progress:summary", {
						completed: completedCount,
						failed: failedCount,
					})}
				</p>
			</div>
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
			<DialogFooter>
				{active ? (
					<>
						<Button
							disabled={!canCancel}
							onClick={() => {
								setCancelRequested(true);
								cancelRef.current();
							}}
						>
							{cancelRequested
								? tc("vpm repositories:import progress:cancelling")
								: tc("general:button:cancel")}
						</Button>
						<Button className="gap-2" onClick={() => dialog.minimize()}>
							<Minimize2 className="size-4" />
							{tc("vpm repositories:import progress:minimize")}
						</Button>
					</>
				) : (
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
						<Button onClick={() => dialog.close()}>
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
		case "running":
			return tc("vpm repositories:import progress:title");
		case "finalizing":
			return tc("vpm repositories:import progress:finalizing");
		case "completed":
			return tc("vpm repositories:import progress:completed");
		case "partial":
			return tc("vpm repositories:import progress:partial");
		case "failed":
			return tc("vpm repositories:import progress:failed");
		case "cancelled":
			return tc("vpm repositories:import progress:cancelled");
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
		case "cancelled":
			return tc("vpm repositories:import progress:status:cancelled");
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
		case "cancelled":
			return "text-warning";
		default:
			return "text-muted-foreground";
	}
}
