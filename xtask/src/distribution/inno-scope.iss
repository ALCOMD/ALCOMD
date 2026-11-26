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
  Key, OwnerKey, OldRuntime, Manifest, Reserved: String;
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
      if PlainSid(Profiles[Index]) and (RegKeyExists(HKEY_USERS, OwnerKey) or
        RegValueExists(HKEY_USERS, Profiles[Index] + '\' + UserScopeLocatorKey, 'OwnedRoot')) then begin
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
  if IsAdminInstallMode then begin
    if not RegQueryStringValue(HKEY_USERS, ScopeRequester + '\' + UserScopeLocatorKey, 'OwnedRoot', Reserved) then
      Exit;
  end else if not ReadScopeRoot(True, Reserved) then
    Exit;
  if CompareText(ScopeSourceRoot, Reserved) <> 0 then
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
  ExitCode, Remaining: Integer;
begin
  Result := 'The existing Windows installation could not be identified for scope conversion.';
  if not DetectScopeSource then
    Exit;
  Approval := ExpandConstant('{param:ALCOMDConvertScope|}');
  if (Approval <> '@MANIFEST_DIGEST@') and
    (SuppressibleMsgBox('Move ALCOMD ' + ScopeSourceVersion + ' from:' + #13#10 + ScopeSourceRoot + #13#10 +
      'To @APP_VERSION@ in:' + #13#10 + Root + #13#10#13#10 +
      'This also moves the owned PATH and Start menu entries. Project files and per-account data are kept.',
      mbConfirmation, MB_YESNO, IDNO) <> IDYES) then begin
    Result := 'ALCOMD kept the existing installation.';
    Exit;
  end;
  Result := EnsureWebView2(NeedsRestart);
  if Result <> '' then
    Exit;

  Result := 'The signed Windows conversion payload could not be prepared.';
  if not ReservePreparedRoot(Root) then
    Exit;
  Parent := ExtractFileDir(Root);
  ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*');
  Extracted := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  // Keep the verified worker separate from the destination so ordinary Setup
  // rollback cannot remove the worker needed to reopen the previous scope.
  ScopeStage := AddBackslash(Parent) + '.@INSTALL_DIRECTORY@-scope-payload-@MANIFEST_DIGEST@';
  if EntryIsMissing(ScopeStage) and not PublishStagedRuntime(Extracted, ScopeStage) then
    Exit;
  Remaining := @RUNTIME_FILE_COUNT@;
  if not RuntimeMatchesStage(ScopeStage, Extracted, 0, Remaining) or (Remaining <> 0) then
    Exit;
  if IsAdminInstallMode then begin
    if not ExecAsOriginalUser(ScopeStage + '\@BOOTSTRAP_FILE@',
      'windows-scope-account-sid --expected ' + ScopeRequester, ScopeStage, SW_HIDE,
      ewWaitUntilTerminated, ExitCode) or (ExitCode <> 0) then begin
      Result := 'Start the scope conversion from the account that owns the existing installation.';
      Exit;
    end;
  end else if not ScopeBootstrapOutput('windows-scope-account-sid', ScopeRequester) then
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
    if not EmptyPreparedRoot(Root) or RegKeyExists(InstallRegistryRoot, UninstallKey) or
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
  if not ScopeDestinationAction('prepare', False) then
    Exit;
  ConvertingScope := True;
  Result := 'Close the previous ALCOMD installation and retry the scope conversion.';
  if not ScopeSourceAction('fence') then
    Exit;
  Result := '';
end;

function FinishScopeConversion: Boolean;
var
  Root, Bootstrap, SourceScope, Extracted: String;
  ExitCode, Remaining: Integer;
begin
  Result := False;
  Root := ExpandConstant('{app}');
  if not OwnedUninstallRoot(Root) or not SyncUninstallRegistration then
    Exit;
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
  if not InstallCliPath then
    Exit;
  PublishStartMenuEntry;
  if not SyncUninstallRegistration then
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
  Remaining := @RUNTIME_FILE_COUNT@;
  Extracted := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  if RuntimeMatchesStage(ScopeStage, Extracted, 0, Remaining) and (Remaining = 0) then
    DelTree(ScopeStage, True, True, True);
end;

procedure RestoreUncommittedScope;
begin
  if ConvertingScope and not ScopeSourceRetired and
    PlainFileMatches(ScopeSourceRoot + '\' + ScopeTransition, ScopePlanDigest) and
    RecordMatches(ScopeSourceRoot + '\' + ActiveRuntime, ScopeSourceVersion + #10) then
    if not ScopeSourceAction('restore') then
      Log('The Windows scope transaction must be recovered with the same installer.');
end;
