import { describe, expect, test } from "vitest";
import { unityButtonView } from "@/lib/unity-project-status";

describe("Unity project button state", () => {
	test("opens a closed project", () => {
		expect(
			unityButtonView({
				status: "Closed",
				can_bring_to_front: false,
			}),
		).toEqual({
			action: "open",
			label: "projects:button:open unity",
			disabled: false,
			showSpinner: false,
		});
	});

	test("disables the button while Unity is opening", () => {
		expect(
			unityButtonView({
				status: "Opening",
				can_bring_to_front: false,
			}),
		).toMatchObject({
			action: "opening",
			label: "projects:button:opening",
			disabled: true,
			showSpinner: true,
		});
	});

	test("brings an open Unity window to the front when supported", () => {
		expect(
			unityButtonView({
				status: "Open",
				can_bring_to_front: true,
			}),
		).toMatchObject({
			action: "bring-to-front",
			label: "projects:button:bring to front",
			disabled: false,
		});
	});

	test("shows an inert open state when activation is unsupported", () => {
		expect(
			unityButtonView({
				status: "Open",
				can_bring_to_front: false,
			}),
		).toMatchObject({
			action: "open-unsupported",
			disabled: true,
		});
	});
});
