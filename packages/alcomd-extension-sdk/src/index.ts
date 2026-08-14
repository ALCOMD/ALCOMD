export interface ExtensionIdentity {
    id: string;
    version: string;
    api: string;
}

export interface ExtensionContext {
    identity: ExtensionIdentity;
    grantedPermissions: readonly string[];
}

export interface ExtensionModule {
    activate(context: ExtensionContext): Promise<void> | void;
    deactivate?(): Promise<void> | void;
}
