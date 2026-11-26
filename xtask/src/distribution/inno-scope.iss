function ScopeBootstrapOutput(Arguments: String; var Text: String): Boolean;
var
  Output: TExecOutput;
  ExitCode: Integer;
begin
  Result := PlainFileMatches(ScopeStage + '\@BOOTSTRAP_FILE@', '@BOOTSTRAP_DIGEST@') and
    ExecAndCaptureOutput(ScopeStage + '\@BOOTSTRAP_FILE@', Arguments, ScopeStage, SW_SHOWNORMAL,
      ewWaitUntilTerminated, ExitCode, Output) and (ExitCode = 0) and not Output.Error and
    (GetArrayLength(Output.StdOut) = 1);
  if Result then
    Text := Output.StdOut[0];
end;

function PlainSid(Value: String): Boolean;
var
  Index: Integer;
begin
  Result := False;
  if (Length(Value) < 5) or (Length(Value) > 184) or (Copy(Value, 1, 4) <> 'S-1-') then
    Exit;
  for Index := 5 to Length(Value) do
    if Pos(Value[Index], '0123456789-') = 0 then
      Exit;
  Result := True;
end;

function DetectScopeSource: Boolean;
var
  Profiles: TArrayOfString;
  RegistryRoot: Cardinal;
  Index: Integer;
  Key, OwnerKey, OldRuntime, Manifest: String;
  ManifestSize: Int64;
begin
  Result := False;
  ScopeRequester := '';
  Key := UninstallKey;
  RegistryRoot := HKEY_LOCAL_MACHINE_64;
  ScopeDestination := 'current-user';
  if IsAdminInstallMode then begin
    ScopeDestination := 'all-users';
    RegistryRoot := HKEY_USERS;
    if not RegGetSubkeyNames(HKEY_USERS, '', Profiles) then
      Exit;
    for Index := 0 to GetArrayLength(Profiles) - 1 do begin
      OwnerKey := Profiles[Index] + '\' + UninstallKey;
      if PlainSid(Profiles[Index]) and RegKeyExists(HKEY_USERS, OwnerKey) then begin
        if ScopeRequester <> '' then
          Exit;
        ScopeRequester := Profiles[Index];
        Key := OwnerKey;
      end;
    end;
    if ScopeRequester = '' then
      Exit;
  end else if RegKeyExists(HKEY_CURRENT_USER, UninstallKey) and
    EntryIsMissing(WizardDirValue + '\scope-destination-v1') then
    Exit;

  if not RegQueryStringValue(RegistryRoot, Key, 'InstallLocation', ScopeSourceRoot) or
    not RegQueryStringValue(RegistryRoot, Key, 'DisplayVersion', ScopeSourceVersion) or
    not RegQueryStringValue(RegistryRoot, Key, 'UninstallString', ScopeUninstaller) or
    not PlainVersion(ScopeSourceVersion) then
    Exit;
  ScopeSourceRoot := RemoveBackslashUnlessRoot(ScopeSourceRoot);
  if (Copy(ScopeUninstaller, 1, 1) = '"') and
    (Copy(ScopeUninstaller, Length(ScopeUninstaller), 1) = '"') then
    ScopeUninstaller := Copy(ScopeUninstaller, 2, Length(ScopeUninstaller) - 2);
  if (Pos('"', ScopeUninstaller) <> 0) or
    (CompareText(ExtractFileDir(ScopeUninstaller), ScopeSourceRoot) <> 0) then
    Exit;
  if not IsAdminInstallMode and
    (CompareText(ScopeSourceRoot, ExpandConstant('{commonpf}\@INSTALL_DIRECTORY@')) <> 0) then
    Exit;
  OldRuntime := ScopeSourceRoot + '\runtime\' + ScopeSourceVersion;
  Manifest := OldRuntime + '\@MANIFEST_NAME@';
  Result := PlainDirectoryChain(OldRuntime, False) and PlainFile(Manifest) and
    FileSize64(Manifest, ManifestSize) and (ManifestSize > 0) and (ManifestSize <= 4194304) and
    ReadDigest(Manifest, ScopeSourceDigest);
end;

function ScopeDestinationAction(Action: String; IncludeSource: Boolean): Boolean;
var
  Arguments: String;
  ExitCode: Integer;
begin
  Arguments := 'coordinate-windows-scope-destination --action ' + Action + ' ' + ScopePlanArguments;
  if IncludeSource then
    Arguments := Arguments + ' --source-root "' + ScopeSourceRoot + '"';
  Result := PlainFileMatches(ScopeStage + '\@BOOTSTRAP_FILE@', '@BOOTSTRAP_DIGEST@') and
    Exec(ScopeStage + '\@BOOTSTRAP_FILE@', Arguments, ScopeStage, SW_HIDE,
      ewWaitUntilTerminated, ExitCode) and (ExitCode = 0);
end;

function ScopeSourceAction(Action: String): Boolean;
var
  Arguments: String;
  ExitCode: Integer;
begin
  Result := False;
  if not PlainFileMatches(ScopeStage + '\@BOOTSTRAP_FILE@', '@BOOTSTRAP_DIGEST@') then
    Exit;
  Arguments := ' --action ' + Action + ' --source-version ' + ScopeSourceVersion +
    ' --destination-version @APP_VERSION@ --staged-runtime "' + ScopeStage +
    '" --approved-plan ' + ScopePlanDigest;
  if IsAdminInstallMode then begin
    Arguments := 'coordinate-windows-scope-source --destination all-users --requester-sid ' +
      ScopeRequester + Arguments;
    Result := ExecAsOriginalUser(ScopeStage + '\@BOOTSTRAP_FILE@', Arguments, ScopeStage,
      SW_HIDE, ewWaitUntilTerminated, ExitCode) and (ExitCode = 0);
  end else begin
    Arguments := 'request-windows-machine-scope-source' + Arguments;
    Result := Exec(ScopeStage + '\@BOOTSTRAP_FILE@', Arguments, ScopeStage, SW_HIDE,
      ewWaitUntilTerminated, ExitCode) and (ExitCode = 0);
  end;
end;

function PrepareScopeConversion(Root: String; var NeedsRestart: Boolean): String;
var
  Parent, Extracted, DestinationRuntime, Approval: String;
  Remaining: Integer;
begin
  Result := 'The existing Windows installation could not be identified for scope conversion.';
  if not DetectScopeSource then
    Exit;
  Approval := ExpandConstant('{param:ALCOMDConvertScope|}');
  if (Approval <> '@MANIFEST_DIGEST@') and
    (SuppressibleMsgBox('Switch the existing ALCOMD ' + ScopeSourceVersion +
      ' installation to the selected scope? Application files and system entries will move. ' +
      'Project files and per-account data will be kept.', mbConfirmation, MB_YESNO, IDNO) <> IDYES) then begin
    Result := 'ALCOMD kept the existing installation.';
    Exit;
  end;
  Result := EnsureWebView2(NeedsRestart);
  if Result <> '' then
    Exit;

  Result := 'The signed Windows conversion payload could not be prepared.';
  Parent := ExtractFileDir(Root);
  ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*');
  Extracted := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  ScopeStage := Extracted;
  if not PlainDirectoryChain(Extracted, False) or
    not PlainFileMatches(Extracted + '\@MANIFEST_NAME@', '@MANIFEST_DIGEST@') then
    Exit;
  if not IsAdminInstallMode and not ScopeBootstrapOutput('windows-scope-account-sid', ScopeRequester) then
    Exit;
  if not PlainSid(ScopeRequester) then
    Exit;

  ScopePlanArguments := '--requester-sid ' + ScopeRequester + ' --destination ' + ScopeDestination +
    ' --source-version ' + ScopeSourceVersion + ' --source-manifest-sha256 ' + ScopeSourceDigest +
    ' --destination-version @APP_VERSION@ --destination-manifest-sha256 @MANIFEST_DIGEST@';
  if not ScopeBootstrapOutput('windows-scope-plan-fingerprint ' + ScopePlanArguments, ScopePlanDigest) or
    not LowerDigest(ScopePlanDigest) then
    Exit;
  ScopeDestinationIntent := AddBackslash(Parent) + '.@INSTALL_DIRECTORY@-scope-' + ScopePlanDigest;
  if EntryIsMissing(ScopeDestinationIntent) then begin
    if not EntryIsMissing(Root) or RegKeyExists(InstallRegistryRoot, UninstallKey) or
      not PublishRecord(ScopeDestinationIntent, ScopePlanDigest + #10) then
      Exit;
  end else if not RecordMatches(ScopeDestinationIntent, ScopePlanDigest + #10) then
    Exit;

  DestinationRuntime := Root + '\runtime\@APP_VERSION@';
  if not PlainDirectoryChain(Root + '\runtime', True) or not ForceDirectories(Root + '\runtime') then
    Exit;
  if EntryIsMissing(DestinationRuntime) and not PublishStagedRuntime(Extracted, DestinationRuntime) then
    Exit;
  Remaining := @RUNTIME_FILE_COUNT@;
  if not RuntimeMatchesStage(DestinationRuntime, Extracted, 0, Remaining) or (Remaining <> 0) then
    Exit;
  ScopeStage := DestinationRuntime;
  if not ScopeDestinationAction('prepare', False) then
    Exit;
  Result := 'Close the previous ALCOMD installation and retry the scope conversion.';
  if not ScopeSourceAction('fence') then
    Exit;
  ConvertingScope := True;
  Result := '';
end;

function FinishScopeConversion: Boolean;
var
  Root, Bootstrap, SourceScope: String;
  ExitCode: Integer;
begin
  Result := False;
  Root := ExpandConstant('{app}');
  if not OwnedUninstallRoot(Root) or not InstallCliPath or not SyncUninstallRegistration then
    Exit;
  PublishStartMenuEntry;
  if not ScopeSourceAction('retire') then
    Exit;
  ScopeSourceRetired := True;
  if not ScopeDestinationAction('activate', True) then
    Exit;
  Bootstrap := Root + '\runtime\@APP_VERSION@\@BOOTSTRAP_FILE@';
  if not PlainFileMatches(Bootstrap, '@BOOTSTRAP_DIGEST@') or
    not ExecAsOriginalUser(Bootstrap, 'complete-windows-scope-account --requester-sid ' +
      ScopeRequester + ' --source-sha @SOURCE_SHA@', ExtractFileDir(Bootstrap), SW_HIDE,
      ewWaitUntilTerminated, ExitCode) or (ExitCode <> 0) then
    Exit;
  SourceScope := 'current-user';
  if not IsAdminInstallMode then
    SourceScope := 'all-users';
  if not PlainFileMatches(ScopeStage + '\@BOOTSTRAP_FILE@', '@BOOTSTRAP_DIGEST@') or
    not ExecAsOriginalUser(ScopeStage + '\@BOOTSTRAP_FILE@',
      'remove-retired-windows-scope --source ' + SourceScope + ' --requester-sid ' +
      ScopeRequester + ' --uninstaller "' + ScopeUninstaller + '" --approved-plan ' + ScopePlanDigest,
      ScopeStage, SW_HIDE, ewWaitUntilTerminated, ExitCode) or (ExitCode <> 0) then
    Exit;
  Result := True;
  if RecordMatches(ScopeDestinationIntent, ScopePlanDigest + #10) then
    DeleteFile(ScopeDestinationIntent);
end;

procedure RestoreUncommittedScope;
begin
  if ConvertingScope and not ScopeSourceRetired then
    if not ScopeSourceAction('restore') then
      Log('The Windows scope transaction must be recovered with the same installer.');
end;
