"use client";

import {
	closestCenter,
	DndContext,
	type DragEndEvent,
	KeyboardSensor,
	PointerSensor,
	useSensor,
	useSensors,
} from "@dnd-kit/core";
import {
	arrayMove,
	SortableContext,
	sortableKeyboardCoordinates,
	useSortable,
	verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
	queryOptions,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
	ArrowUpDown,
	Blocks,
	Eye,
	EyeOff,
	GripVertical,
	RotateCcw,
} from "lucide-react";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { HNavBar, HNavBarText, VStack } from "@/components/layout";
import { ScrollPageContainer } from "@/components/ScrollPageContainer";
import { SIDEBAR_EXTENSION_DEFINITIONS } from "@/components/sidebar-extension-definitions";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
	Dialog,
	DialogClose,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import {
	commands,
	type ExtensionManagementInfo,
	type SidebarExtension,
} from "@/lib/bindings";
import { tc, tt } from "@/lib/i18n";
import {
	DEFAULT_SIDEBAR_EXTENSION_ORDER,
	EXTENSION_MANAGEMENT_QUERY_KEY,
	restoreDefaultSidebarExtensions,
	SIDEBAR_EXTENSIONS_QUERY_KEY,
} from "@/lib/sidebar-extensions";
import { toastThrownError } from "@/lib/toast";

export const Route = createFileRoute("/_main/extensions/")({
	component: ExtensionsPage,
});

function ExtensionsPage() {
	return (
		<VStack>
			<HNavBar
				className="shrink-0"
				leading={<HNavBarText>{tc("extensions")}</HNavBarText>}
			/>
			<ScrollPageContainer viewportClassName="rounded-xl shadow-xl h-full">
				<main className="flex w-full flex-col gap-3 p-2 compact:p-1">
					<ExtensionsSortCard />
					<ExtensionsManageCard />
				</main>
			</ScrollPageContainer>
		</VStack>
	);
}

const SIDEBAR_EXTENSIONS_QUERY = queryOptions({
	queryKey: SIDEBAR_EXTENSIONS_QUERY_KEY,
	queryFn: commands.environmentGetSidebarExtensions,
	initialData: DEFAULT_SIDEBAR_EXTENSION_ORDER.map((id) => ({
		id,
		installed: true,
		enabled: true,
		visible: true,
	})),
});

const EXTENSION_MANAGEMENT_QUERY = queryOptions({
	queryKey: EXTENSION_MANAGEMENT_QUERY_KEY,
	queryFn: commands.environmentGetExtensionManagement,
	initialData: [] as ExtensionManagementInfo[],
});

function extensionLabel(id: string) {
	const definition = SIDEBAR_EXTENSION_DEFINITIONS[id];
	if (!definition) return id;
	return tc(definition.labelKey);
}

function extensionManagementLabel(extension: ExtensionManagementInfo) {
	const definition = SIDEBAR_EXTENSION_DEFINITIONS[extension.id];
	if (!definition) return extension.displayName;
	return tc(definition.labelKey);
}

function useSidebarExtensions() {
	return useQuery(SIDEBAR_EXTENSIONS_QUERY);
}

function useExtensionManagement() {
	return useQuery(EXTENSION_MANAGEMENT_QUERY);
}

function isSortableSidebarExtension(extension: SidebarExtension) {
	return extension.installed;
}

function mergeSidebarExtensionOrder(
	extensions: SidebarExtension[],
	orderedExtensions: SidebarExtension[],
) {
	const orderedIterator = orderedExtensions[Symbol.iterator]();
	return extensions.map((extension) =>
		isSortableSidebarExtension(extension)
			? (orderedIterator.next().value ?? extension)
			: extension,
	);
}

function hasSameSidebarExtensionOrder(
	left: SidebarExtension[],
	right: SidebarExtension[],
) {
	return (
		left.length === right.length &&
		left.every((extension, index) => extension.id === right[index]?.id)
	);
}

function hasSameSidebarExtensions(
	left: SidebarExtension[],
	right: SidebarExtension[],
) {
	return (
		hasSameSidebarExtensionOrder(left, right) &&
		left.every((extension, index) => {
			const other = right[index];
			return (
				other != null &&
				extension.installed === other.installed &&
				extension.enabled === other.enabled &&
				extension.visible === other.visible
			);
		})
	);
}

function ExtensionsSortCard() {
	const queryClient = useQueryClient();
	const extensionsQuery = useSidebarExtensions();
	const [sortDialogOpen, setSortDialogOpen] = useState(false);
	const [restoreDefaultRequested, setRestoreDefaultRequested] = useState(false);
	const sensors = useSensors(
		useSensor(PointerSensor, {
			activationConstraint: {
				distance: 4,
			},
		}),
		useSensor(KeyboardSensor, {
			coordinateGetter: sortableKeyboardCoordinates,
		}),
	);
	const sortableExtensions = useMemo(
		() => (extensionsQuery.data ?? []).filter(isSortableSidebarExtension),
		[extensionsQuery.data],
	);
	const [orderedExtensions, setOrderedExtensions] =
		useState<SidebarExtension[]>(sortableExtensions);

	useEffect(() => {
		if (!sortDialogOpen) return;
		setOrderedExtensions(sortableExtensions);
		setRestoreDefaultRequested(false);
	}, [sortDialogOpen, sortableExtensions]);

	const sidebarExtensions = extensionsQuery.data ?? [];
	const defaultSidebarExtensions = useMemo(
		() => restoreDefaultSidebarExtensions(sidebarExtensions),
		[sidebarExtensions],
	);
	const defaultOrderedExtensions = useMemo(
		() => defaultSidebarExtensions.filter(isSortableSidebarExtension),
		[defaultSidebarExtensions],
	);
	const pendingSidebarExtensions = useMemo(
		() =>
			mergeSidebarExtensionOrder(
				restoreDefaultRequested ? defaultSidebarExtensions : sidebarExtensions,
				orderedExtensions,
			),
		[
			defaultSidebarExtensions,
			orderedExtensions,
			restoreDefaultRequested,
			sidebarExtensions,
		],
	);
	const hasPendingChange = !hasSameSidebarExtensions(
		sidebarExtensions,
		pendingSidebarExtensions,
	);
	const isDefaultState = useMemo(
		() =>
			hasSameSidebarExtensions(
				pendingSidebarExtensions,
				defaultSidebarExtensions,
			),
		[defaultSidebarExtensions, pendingSidebarExtensions],
	);

	const reorderSidebarExtensions = useMutation({
		mutationFn: async (next: SidebarExtension[]) => {
			await commands.environmentSetSidebarExtensionOrder(next);
		},
		onMutate: async (next) => {
			await queryClient.cancelQueries({
				queryKey: SIDEBAR_EXTENSIONS_QUERY.queryKey,
			});
			const previous = queryClient.getQueryData<SidebarExtension[]>(
				SIDEBAR_EXTENSIONS_QUERY.queryKey,
			);
			queryClient.setQueryData(SIDEBAR_EXTENSIONS_QUERY.queryKey, next);
			return { previous };
		},
		onError: (error, _next, context) => {
			toastThrownError(error);
			if (context?.previous) {
				queryClient.setQueryData(
					SIDEBAR_EXTENSIONS_QUERY.queryKey,
					context.previous,
				);
			}
		},
		onSuccess: () => {
			setSortDialogOpen(false);
		},
		onSettled: () => {
			queryClient.invalidateQueries({
				queryKey: SIDEBAR_EXTENSIONS_QUERY.queryKey,
			});
		},
	});

	const handleDragEnd = ({ active, over }: DragEndEvent) => {
		if (!over || active.id === over.id) return;
		setOrderedExtensions((current) => {
			const oldIndex = current.findIndex(
				(extension) => extension.id === active.id,
			);
			const newIndex = current.findIndex(
				(extension) => extension.id === over.id,
			);
			if (oldIndex < 0 || newIndex < 0) return current;
			return arrayMove(current, oldIndex, newIndex);
		});
	};

	return (
		<Card className="p-4 compact:p-3">
			<h2 className="mb-2 text-lg">{tc("extensions")}</h2>
			<p className="text-sm whitespace-normal">
				{tc("extensions:description")}
			</p>
			<div className="mt-3">
				<Dialog open={sortDialogOpen} onOpenChange={setSortDialogOpen}>
					<DialogTrigger asChild>
						<Button className={"compact:h-10"}>
							<ArrowUpDown className="mr-2 size-4" />
							{tc("extensions:button:sort sidebar")}
						</Button>
					</DialogTrigger>
					<DialogContent className={"max-w-[600px]"}>
						<DialogHeader>
							<DialogTitle>{tc("extensions:dialog:sort sidebar")}</DialogTitle>
						</DialogHeader>
						<p className="text-sm whitespace-normal">
							{tc("extensions:dialog:sort sidebar description")}
						</p>
						<DndContext
							sensors={sensors}
							collisionDetection={closestCenter}
							onDragEnd={handleDragEnd}
						>
							<SortableContext
								items={orderedExtensions.map((extension) => extension.id)}
								strategy={verticalListSortingStrategy}
							>
								<div className="mt-3 flex flex-col gap-2">
									{orderedExtensions.map((extension) => (
										<SortableExtensionItem
											key={extension.id}
											extension={extension}
											disabled={reorderSidebarExtensions.isPending}
											onVisibilityChange={(visible) =>
												setOrderedExtensions((current) =>
													current.map((item) =>
														item.id === extension.id
															? { ...item, visible }
															: item,
													),
												)
											}
										/>
									))}
								</div>
							</SortableContext>
						</DndContext>
						<DialogFooter>
							<Button
								variant="secondary"
								className="sm:mr-auto"
								onClick={() => {
									setOrderedExtensions(defaultOrderedExtensions);
									setRestoreDefaultRequested(true);
								}}
								disabled={reorderSidebarExtensions.isPending || isDefaultState}
							>
								<RotateCcw className="mr-2 size-4" />
								{tc("extensions:button:restore default order")}
							</Button>
							<DialogClose asChild>
								<Button>{tc("general:button:cancel")}</Button>
							</DialogClose>
							<Button
								onClick={() => {
									reorderSidebarExtensions.mutate(pendingSidebarExtensions);
								}}
								disabled={
									reorderSidebarExtensions.isPending || !hasPendingChange
								}
							>
								{tc("extensions:button:save")}
							</Button>
						</DialogFooter>
					</DialogContent>
				</Dialog>
			</div>
		</Card>
	);
}

function SortableExtensionItem({
	extension,
	disabled,
	onVisibilityChange,
}: {
	extension: SidebarExtension;
	disabled: boolean;
	onVisibilityChange: (visible: boolean) => void;
}) {
	const {
		attributes,
		listeners,
		setNodeRef,
		setActivatorNodeRef,
		transform,
		transition,
		isDragging,
	} = useSortable({
		id: extension.id,
		disabled,
	});
	const Icon = SIDEBAR_EXTENSION_DEFINITIONS[extension.id]?.icon ?? Blocks;
	const isShown = extension.enabled && extension.visible;
	const VisibilityIcon = isShown ? Eye : EyeOff;

	return (
		<div
			ref={setNodeRef}
			style={{
				transform: CSS.Transform.toString(transform),
				transition,
			}}
			className={`flex items-center justify-between gap-3 rounded-md border border-border bg-secondary/30 px-3 py-2 ${
				isDragging ? "z-10 opacity-70 shadow-lg" : ""
			} ${isShown ? "" : "text-muted-foreground"}`}
		>
			<div className="flex min-w-0 items-center gap-3">
				<Icon className="size-5 shrink-0 text-primary" />
				<p className="truncate font-normal">{extensionLabel(extension.id)}</p>
			</div>
			<div className="flex shrink-0 items-center gap-1">
				<Button
					variant={isShown ? "secondary" : "ghost"}
					size="icon"
					disabled={disabled || !extension.enabled}
					aria-pressed={isShown}
					aria-label={tt(
						isShown
							? "extensions:button:hide from sidebar"
							: "extensions:button:show in sidebar",
					)}
					onClick={() => onVisibilityChange(!extension.visible)}
				>
					<VisibilityIcon className="size-5" />
				</Button>
				<Button
					ref={setActivatorNodeRef}
					variant="ghost"
					size="icon"
					disabled={disabled}
					aria-label={tt("extensions:button:drag to reorder")}
					className="cursor-grab touch-none active:cursor-grabbing"
					{...attributes}
					{...listeners}
				>
					<GripVertical className="size-5" />
				</Button>
			</div>
		</div>
	);
}

function ExtensionsManageCard() {
	const queryClient = useQueryClient();
	const extensionsQuery = useExtensionManagement();

	const setEnabled = useMutation({
		mutationFn: async ({ id, enabled }: { id: string; enabled: boolean }) =>
			await commands.environmentSetExtensionEnabled(id, enabled),
		onMutate: async ({ id, enabled }) => {
			await Promise.all([
				queryClient.cancelQueries({
					queryKey: EXTENSION_MANAGEMENT_QUERY.queryKey,
				}),
				queryClient.cancelQueries({
					queryKey: SIDEBAR_EXTENSIONS_QUERY.queryKey,
				}),
			]);
			const previousManagement = queryClient.getQueryData<
				ExtensionManagementInfo[]
			>(EXTENSION_MANAGEMENT_QUERY.queryKey);
			const previousSidebar = queryClient.getQueryData<SidebarExtension[]>(
				SIDEBAR_EXTENSIONS_QUERY.queryKey,
			);
			queryClient.setQueryData<ExtensionManagementInfo[]>(
				EXTENSION_MANAGEMENT_QUERY.queryKey,
				(current) =>
					current?.map((extension) =>
						extension.id === id ? { ...extension, enabled } : extension,
					),
			);
			queryClient.setQueryData<SidebarExtension[]>(
				SIDEBAR_EXTENSIONS_QUERY.queryKey,
				(current) =>
					current?.map((extension) =>
						extension.id === id ? { ...extension, enabled } : extension,
					),
			);
			return { previousManagement, previousSidebar };
		},
		onError: (error, _args, context) => {
			toastThrownError(error);
			if (context?.previousManagement) {
				queryClient.setQueryData(
					EXTENSION_MANAGEMENT_QUERY.queryKey,
					context.previousManagement,
				);
			}
			if (context?.previousSidebar) {
				queryClient.setQueryData(
					SIDEBAR_EXTENSIONS_QUERY.queryKey,
					context.previousSidebar,
				);
			}
		},
		onSettled: () => {
			queryClient.invalidateQueries({
				queryKey: EXTENSION_MANAGEMENT_QUERY.queryKey,
			});
			queryClient.invalidateQueries({
				queryKey: SIDEBAR_EXTENSIONS_QUERY.queryKey,
			});
		},
	});

	const extensions = extensionsQuery.data ?? [];
	const installedExtensions = extensions.filter(
		(extension) => extension.installed,
	);
	const uninstalledExtensions = extensions.filter(
		(extension) => !extension.installed,
	);
	const isBusy = setEnabled.isPending;

	return (
		<Card className="p-4 compact:p-3">
			<h2 className="mb-2 text-lg">{tc("extensions:manage")}</h2>
			<p className="text-sm whitespace-normal">
				{tc("extensions:manage description")}
			</p>
			<div className="mt-5 flex flex-col gap-6">
				<ExtensionSection
					title={tc("extensions:installed")}
					emptyText={tc("extensions:empty installed")}
					extensions={installedExtensions}
					renderExtension={(extension) => (
						<ExtensionManageItem
							key={extension.id}
							extension={extension}
							action={
								extension.builtIn ? (
									<span className="rounded-full bg-secondary px-3 py-1 text-sm text-secondary-foreground">
										{tc("extensions:built in")}
									</span>
								) : null
							}
							trailing={
								<div className="flex items-center gap-2">
									<span className="text-sm text-muted-foreground">
										{tc("extensions:enabled")}
									</span>
									<Switch
										checked={extension.enabled}
										disabled={isBusy || !extension.canDisable}
										aria-label={tt("extensions:enabled")}
										onCheckedChange={(enabled) =>
											setEnabled.mutate({
												id: extension.id,
												enabled,
											})
										}
									/>
								</div>
							}
						/>
					)}
				/>
				<ExtensionSection
					title={tc("extensions:not installed")}
					emptyText={tc("extensions:empty not installed")}
					extensions={uninstalledExtensions}
					renderExtension={(extension) => (
						<ExtensionManageItem key={extension.id} extension={extension} />
					)}
				/>
			</div>
		</Card>
	);
}

function ExtensionSection({
	title,
	emptyText,
	extensions,
	renderExtension,
}: {
	title: ReactNode;
	emptyText: ReactNode;
	extensions: ExtensionManagementInfo[];
	renderExtension: (extension: ExtensionManagementInfo) => ReactNode;
}) {
	return (
		<section>
			<div className="mb-3 flex items-center gap-2">
				<h3 className="text-base font-medium">{title}</h3>
				<span className="rounded-full bg-secondary px-2 py-0.5 text-xs text-secondary-foreground">
					{extensions.length}
				</span>
			</div>
			{extensions.length > 0 ? (
				<div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
					{extensions.map(renderExtension)}
				</div>
			) : (
				<div className="rounded-lg border border-dashed border-border px-4 py-6 text-center text-sm text-muted-foreground">
					{emptyText}
				</div>
			)}
		</section>
	);
}

function ExtensionManageItem({
	extension,
	action,
	trailing,
}: {
	extension: ExtensionManagementInfo;
	action?: ReactNode;
	trailing?: ReactNode;
}) {
	const Icon = SIDEBAR_EXTENSION_DEFINITIONS[extension.id]?.icon ?? Blocks;

	return (
		<Card className="flex min-h-40 flex-col gap-5 bg-secondary/25 p-4 shadow-sm">
			<div className="flex min-w-0 items-start gap-3">
				<div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
					<Icon className="size-5" />
				</div>
				<div className="min-w-0">
					<h4 className="truncate font-medium">
						{extensionManagementLabel(extension)}
					</h4>
					<p className="mt-1 truncate text-sm text-muted-foreground">
						{extension.id}
					</p>
				</div>
			</div>
			<div className="mt-auto flex flex-wrap items-center justify-between gap-3">
				{action}
				{trailing}
			</div>
		</Card>
	);
}
