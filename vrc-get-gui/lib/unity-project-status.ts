import type { TauriUnityProjectStatus } from "@/lib/bindings";

export type UnityButtonView =
	| {
			action: "open";
			label: "projects:button:open unity";
			disabled: false;
			showSpinner: false;
	  }
	| {
			action: "opening";
			label: "projects:button:opening";
			disabled: true;
			showSpinner: true;
	  }
	| {
			action: "bring-to-front";
			label: "projects:button:bring to front";
			disabled: false;
			showSpinner: false;
	  }
	| {
			action: "open-unsupported";
			label: "projects:button:unity is open";
			disabled: true;
			showSpinner: false;
	  };

export function unityButtonView(
	status: TauriUnityProjectStatus | undefined,
): UnityButtonView {
	switch (status?.status) {
		case "Opening":
			return {
				action: "opening",
				label: "projects:button:opening",
				disabled: true,
				showSpinner: true,
			};
		case "Open":
			return status.can_bring_to_front
				? {
						action: "bring-to-front",
						label: "projects:button:bring to front",
						disabled: false,
						showSpinner: false,
					}
				: {
						action: "open-unsupported",
						label: "projects:button:unity is open",
						disabled: true,
						showSpinner: false,
					};
		default:
			return {
				action: "open",
				label: "projects:button:open unity",
				disabled: false,
				showSpinner: false,
			};
	}
}
