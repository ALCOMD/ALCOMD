"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
	CheckCircle2,
	CircleAlert,
	Clock3,
	Folder,
	Layers3,
	Monitor,
	PowerOff,
	RadioTower,
	RefreshCw,
	ShieldCheck,
} from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { HNavBar, HNavBarText, VStack } from "@/components/layout";
import { ScrollPageContainer } from "@/components/ScrollPageContainer";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import type { UnityDiscordActivity, UnityDiscordStatus } from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { tc, tt } from "@/lib/i18n";
import { toastThrownError } from "@/lib/toast";
import alcomd3IconUrl from "../../../icons/discord-assets/alcomd3-512.png";
import unityIconUrl from "../../../icons/discord-assets/unity-512.png";

export const Route = createFileRoute("/_main/unity-discord-status/")({
	component: Page,
});

const STATUS_QUERY_KEY = ["unityDiscordStatus"] as const;
const STATUS_REFETCH_INTERVAL_MS = 2_000;

function Page() {
	const queryClient = useQueryClient();
	const status = useQuery({
		queryKey: STATUS_QUERY_KEY,
		queryFn: commands.unityDiscordStatus,
		refetchInterval: STATUS_REFETCH_INTERVAL_MS,
	});
	const refresh = useMutation({
		mutationFn: () =>
			queryClient.invalidateQueries({ queryKey: STATUS_QUERY_KEY }),
	});
	const setSharingEnabled = useMutation({
		mutationFn: commands.unityDiscordSetSharingEnabled,
		onSuccess: (updatedStatus) => {
			queryClient.setQueryData(STATUS_QUERY_KEY, updatedStatus);
		},
		onError: (error) => {
			console.error(error);
			toastThrownError(error);
		},
	});

	return (
		<VStack>
			<HNavBar
				leading={<HNavBarText>{tc("unity discord:title")}</HNavBarText>}
				trailing={
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="ghost"
								className="compact:h-10"
								disabled={refresh.isPending}
								onClick={() => refresh.mutate()}
								aria-label={tt("unity discord:refresh")}
							>
								<RefreshCw
									className={`size-5 ${refresh.isPending ? "animate-spin" : ""}`}
								/>
							</Button>
						</TooltipTrigger>
						<TooltipContent>{tc("unity discord:refresh")}</TooltipContent>
					</Tooltip>
				}
			/>
			<ScrollPageContainer viewportClassName="rounded-xl shadow-xl h-full">
				<main className="grid min-w-0 gap-3 whitespace-normal pb-1">
					{status.data ? (
						<>
							<StatusCard
								status={status.data}
								updating={setSharingEnabled.isPending}
								setSharingEnabled={(enabled) =>
									setSharingEnabled.mutate(enabled)
								}
							/>
							<div className="grid items-start gap-3 xl:grid-cols-[minmax(0,1.15fr)_minmax(20rem,0.85fr)]">
								<DiscordPreviewCard activity={status.data.activity} />
								<SharedDataCard />
							</div>
						</>
					) : (
						<Card className="p-5 text-muted-foreground">
							{tc("unity discord:loading")}
						</Card>
					)}
				</main>
			</ScrollPageContainer>
		</VStack>
	);
}

function StatusCard({
	status,
	updating,
	setSharingEnabled,
}: {
	status: UnityDiscordStatus;
	updating: boolean;
	setSharingEnabled: (enabled: boolean) => void;
}) {
	const presentation = getStatusPresentation(status);
	const StatusIcon = presentation.icon;

	return (
		<Card className="p-5 compact:p-4">
			<div className="flex flex-wrap items-start gap-4">
				<div
					className={`flex size-12 shrink-0 items-center justify-center rounded-2xl ${presentation.iconClassName}`}
				>
					<StatusIcon className="size-6" />
				</div>
				<div className="min-w-0 basis-60 grow">
					<h2 className="font-medium">{tc(presentation.titleKey)}</h2>
					<p className="mt-1 max-w-3xl text-sm text-muted-foreground">
						{tc(presentation.descriptionKey)}
					</p>
				</div>
				<label
					className="flex min-w-64 items-center gap-4 rounded-xl bg-secondary/60 px-3 py-2.5"
					htmlFor="unity-discord-sharing"
				>
					<span className="min-w-0 grow">
						<span className="block text-sm font-medium">
							{tc("unity discord:sharing:title")}
						</span>
						<span className="mt-0.5 block text-xs text-muted-foreground">
							{tc("unity discord:sharing:description")}
						</span>
					</span>
					<Switch
						id="unity-discord-sharing"
						checked={status.sharingEnabled}
						disabled={updating}
						onCheckedChange={setSharingEnabled}
					/>
				</label>
			</div>
			<div className="mt-5 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
				<StatusMetric
					label={tc("unity discord:metric:extension")}
					value={
						status.workerRunning
							? tc("unity discord:value:running")
							: tc("unity discord:value:stopped")
					}
					active={status.workerRunning}
				/>
				<StatusMetric
					label={tc("unity discord:metric:sharing")}
					value={
						status.sharingEnabled
							? tc("unity discord:value:on")
							: tc("unity discord:value:off")
					}
					active={status.sharingEnabled}
				/>
				<StatusMetric
					label={tc("unity discord:metric:unity")}
					value={
						status.activity
							? tc("unity discord:value:editors", {
									count: status.activity.editorCount,
								})
							: tc("unity discord:value:not detected")
					}
					active={status.activity != null}
				/>
				<StatusMetric
					label={tc("unity discord:metric:discord")}
					value={
						status.discordConnected
							? tc("unity discord:value:connected")
							: tc("unity discord:value:not connected")
					}
					active={status.discordConnected}
				/>
			</div>
		</Card>
	);
}

function getStatusPresentation(status: UnityDiscordStatus) {
	if (!status.enabled) {
		return {
			titleKey: "unity discord:status:disabled",
			descriptionKey: "unity discord:status:disabled description",
			icon: PowerOff,
			iconClassName: "bg-secondary text-muted-foreground",
		};
	}
	if (!status.sharingEnabled) {
		return {
			titleKey: "unity discord:status:sharing disabled",
			descriptionKey: "unity discord:status:sharing disabled description",
			icon: ShieldCheck,
			iconClassName: "bg-secondary text-muted-foreground",
		};
	}
	if (!status.applicationConfigured) {
		return {
			titleKey: "unity discord:status:not configured",
			descriptionKey: "unity discord:status:not configured description",
			icon: CircleAlert,
			iconClassName: "bg-warning/15 text-warning",
		};
	}
	if (status.discordConnected) {
		return {
			titleKey: "unity discord:status:connected",
			descriptionKey: "unity discord:status:connected description",
			icon: CheckCircle2,
			iconClassName: "bg-success/15 text-success",
		};
	}
	if (status.activity) {
		return {
			titleKey: "unity discord:status:discord unavailable",
			descriptionKey: "unity discord:status:discord unavailable description",
			icon: CircleAlert,
			iconClassName: "bg-warning/15 text-warning",
		};
	}
	return {
		titleKey: "unity discord:status:waiting",
		descriptionKey: "unity discord:status:waiting description",
		icon: RadioTower,
		iconClassName: "bg-primary/10 text-primary",
	};
}

function StatusMetric({
	label,
	value,
	active,
}: {
	label: ReactNode;
	value: ReactNode;
	active: boolean;
}) {
	return (
		<div className="rounded-xl bg-secondary/60 px-3 py-2.5">
			<p className="text-xs text-muted-foreground">{label}</p>
			<div className="mt-1 flex items-center gap-2 text-sm font-medium">
				<span
					className={`size-2 rounded-full ${active ? "bg-success" : "bg-muted-foreground/45"}`}
					aria-hidden="true"
				/>
				{value}
			</div>
		</div>
	);
}

function DiscordPreviewCard({
	activity,
}: {
	activity: UnityDiscordActivity | null;
}) {
	const elapsed = useElapsedTime(activity?.startedAt ?? null);
	const details = activity
		? `Editing ${activity.projectName}`
		: tc("unity discord:preview:waiting");
	const editor = activity?.unityVersion
		? `Unity ${activity.unityVersion}`
		: "Unity Editor";
	const state =
		activity && activity.editorCount > 1
			? `${editor} · ${activity.editorCount} editors open`
			: editor;

	return (
		<Card className="p-5 compact:p-4">
			<div className="mb-4 flex items-center gap-2">
				<Monitor className="size-5 text-primary" />
				<h2 className="font-medium">{tc("unity discord:preview:title")}</h2>
			</div>
			<div className="rounded-2xl bg-inverse-surface p-4 text-inverse-on-surface shadow-sm">
				<p className="text-[0.68rem] font-semibold tracking-wider opacity-65">
					{tc("unity discord:preview:activity")}
				</p>
				<div className="mt-3 flex min-w-0 gap-3">
					<div className="relative size-16 shrink-0 rounded-xl bg-white p-1.5 shadow-sm">
						<img
							src={unityIconUrl}
							alt=""
							className="size-full object-contain"
						/>
						<span className="absolute -right-1 -bottom-1 flex size-6 items-center justify-center rounded-full bg-inverse-surface p-0.5 ring-2 ring-inverse-surface">
							<img
								src={alcomd3IconUrl}
								alt=""
								className="size-full object-contain"
							/>
						</span>
					</div>
					<div className="min-w-0 pt-0.5 text-sm">
						<h3 className="truncate font-semibold">Unity</h3>
						<p className="truncate opacity-90">{details}</p>
						<p className="truncate opacity-90">{activity ? state : "—"}</p>
						{activity && (
							<p className="mt-1 flex items-center gap-1.5 opacity-75">
								<Clock3 className="size-3.5" />
								{elapsed}
							</p>
						)}
					</div>
				</div>
			</div>
			<p className="mt-3 text-sm text-muted-foreground">
				{tc("unity discord:preview:description")}
			</p>
		</Card>
	);
}

function SharedDataCard() {
	const items = [
		{ icon: Folder, labelKey: "unity discord:data:project name" },
		{ icon: Monitor, labelKey: "unity discord:data:unity version" },
		{ icon: Layers3, labelKey: "unity discord:data:editor count" },
		{ icon: Clock3, labelKey: "unity discord:data:session duration" },
	];

	return (
		<Card className="p-5 compact:p-4">
			<div className="mb-4 flex items-center gap-2">
				<ShieldCheck className="size-5 text-primary" />
				<h2 className="font-medium">{tc("unity discord:data:title")}</h2>
			</div>
			<ul className="grid gap-2">
				{items.map(({ icon: Icon, labelKey }) => (
					<li
						key={labelKey}
						className="flex items-center gap-3 rounded-xl bg-secondary/60 px-3 py-2.5 text-sm"
					>
						<Icon className="size-4 shrink-0 text-primary" />
						{tc(labelKey)}
					</li>
				))}
			</ul>
			<div className="mt-3 flex gap-2 rounded-xl border border-primary/20 bg-primary/5 p-3 text-sm text-muted-foreground">
				<ShieldCheck className="mt-0.5 size-4 shrink-0 text-primary" />
				<p className="min-w-0 whitespace-normal break-words">
					{tc("unity discord:data:privacy")}
				</p>
			</div>
		</Card>
	);
}

function useElapsedTime(startedAt: number | null) {
	const [now, setNow] = useState(Date.now());

	useEffect(() => {
		if (startedAt == null) return;
		setNow(Date.now());
		const timer = window.setInterval(() => setNow(Date.now()), 1_000);
		return () => window.clearInterval(timer);
	}, [startedAt]);

	if (startedAt == null) return "";
	return formatElapsedTime(Math.max(0, Math.floor(now / 1_000) - startedAt));
}

export function formatElapsedTime(totalSeconds: number) {
	const hours = Math.floor(totalSeconds / 3_600);
	const minutes = Math.floor((totalSeconds % 3_600) / 60);
	const seconds = totalSeconds % 60;
	return [hours, minutes, seconds]
		.map((value) => value.toString().padStart(2, "0"))
		.join(":");
}
