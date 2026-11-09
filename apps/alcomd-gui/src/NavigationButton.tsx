import { Button } from "./Material";
import { useRoutePreload } from "./Preload";

export function NavigationButton({ to, ...props }: Parameters<typeof Button>[0] & { to: string }) {
    const { pin: _pin, prepare: _prepare, ...intent } = useRoutePreload(to, !props.disabled);
    return <Button {...props} {...intent} />;
}
