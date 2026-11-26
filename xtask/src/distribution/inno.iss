[Setup]
AppId={@APP_ID@
AppName=@APP_NAME@
AppVersion=@APP_VERSION@
AppPublisher=@APP_PUBLISHER@
DefaultDirName={code:DefaultProductRoot}
DisableDirPage=no
DisableProgramGroupPage=yes
AllowNoIcons=yes
UsePreviousAppDir=no
UsePreviousPrivileges=no
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupMutex=Global\@FAMILY_NAME@.ProductSetup
CloseApplications=no
RestartApplications=no
ChangesEnvironment=yes
UninstallDisplayIcon={app}\runtime\@APP_VERSION@\@GUI_NAME@.exe
Uninstallable=not IsRepairInstallation
OutputBaseFilename=@FAMILY_NAME@_@APP_VERSION@_windows_x86_64_setup

[Files]
@FILES@
Source: "active-runtime-v1"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall; Check: IsInitialActivation

[Icons]
Name: "{app}\start-menu"; Filename: "{app}\runtime\@APP_VERSION@\@GUI_NAME@.exe"; AppUserModelID: "@APP_AUMID@"; Flags: createonlyiffileexists; Check: CreateFreshIcon
Name: "{tmp}\candidate-start-menu"; Filename: "{app}\runtime\@APP_VERSION@\@GUI_NAME@.exe"; AppUserModelID: "@APP_AUMID@"; Flags: createonlyiffileexists uninsneveruninstall; Check: CreateUpdateIcon

[UninstallDelete]
@UNINSTALL_FILES@

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\@APP_ID@_is1';
  UserScopeLocatorKey = 'Software\@APP_PUBLISHER@\@FAMILY_NAME@\WindowsInstaller';
  RootLocator = '.@INSTALL_DIRECTORY@-install-root-v1';
  RootUninstallGuard = '.@INSTALL_DIRECTORY@-uninstall-root-v1';
  ActiveRuntime = 'active-runtime-v1';
  UpdateIntent = 'update-intent-v1';
  ScopeTransition = 'scope-transition-v1';
  ScopeDestinationPrepared = 'scope-destination-v1';
  ScopeDestinationCommit = 'scope-destination-commit-v1';
  ShortcutSnapshot = 'previous-start-menu-v1';
  UpdateInstalled = 'installed-v1';
  PreviousShortcutFile = 'previous-start-menu.lnk';
  MachineBridgeAccount = '@MACHINE_BRIDGE_ACCOUNT@';
  CandidateShortcutFile = 'candidate-start-menu.lnk';
  MissingFile = 2;
  MissingPath = 3;
  LockViolation = 33;
  FixedDrive = 3;
  InvalidFileAttributes = $FFFFFFFF;
  ReplaceExisting = 1;
  WriteThrough = 8;
  UserEnvironment = 'Environment';
  MachineEnvironment = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';
  // These claims live in the reserved root and survive loss of the stock Inno key.
  CliPathReceipt = 'cli-path-owner-v1';
  StartMenuReceipt = 'start-menu-owner-v1';
  ProductProtocolKey = 'Software\Classes\@URI_SCHEME@';
  ProductProtocolOwner = '@FAMILY_NAME@.InstallationRoot';
  ProductProtocolDescription = 'URL:@APP_NAME@ protocol';
  StringValue = 1;
  ExpandStringValue = 2;
  ReadEnvironmentString = $10000006;
  KeyQueryValue = 1;
  Registry64BitView = $0100;
  IncompleteInstallation = 9;

type
  TLeaseRange = record
    Internal, InternalHigh, Offset, OffsetHigh, EventHandle: Cardinal;
  end;

var
  ProgramLease: Integer;
  CommittingUpdate, UpdateEntriesReady, Repairing, RebuildUninstallRegistration, CompletingBridge: Boolean;
  BridgeJournal, BridgeStage, BridgeExtracted, BridgeIntent, BridgeProof: String;
  ConvertingScope, ScopeSourceRetired: Boolean;
  InstallationIncomplete: Boolean;
  PreviousVersion, PreviousDigest, UpdateJournal, UpdateStage: String;
  PreviousShortcut, RepairStage: String;
  ScopeStage, ScopeSourceRoot, ScopeSourceVersion, ScopeSourceDigest, ScopeUninstaller: String;
  ScopeDestination, ScopeRequester, ScopePlanArguments, ScopePlanDigest: String;
  ScopeDestinationIntent: String;
  RetiredUninstallDigest: String;
  UninstallScopePlan: String;
  InstallRootWorker: String;
  UninstallRootGuardPath: String;

function OpenLeaseFile(FileName: String; Access, Share: Cardinal; Security: Integer;
  Disposition, Flags: Cardinal; Template: Integer): Integer;
  external 'CreateFileW@kernel32.dll stdcall';

function TryLockLease(Handle: Integer; Flags, Reserved, LengthLow, LengthHigh: Cardinal;
  var Range: TLeaseRange): Boolean;
  external 'LockFileEx@kernel32.dll stdcall';

function CloseKernelHandle(Handle: Integer): Boolean;
  external 'CloseHandle@kernel32.dll stdcall';

function FlushFile(Handle: Integer): Boolean;
  external 'FlushFileBuffers@kernel32.dll stdcall';

function PublishFile(Source, Destination: String; Flags: Cardinal): Boolean;
  external 'MoveFileExW@kernel32.dll stdcall';

function GetAttributes(FileName: String): Cardinal;
  external 'GetFileAttributesW@kernel32.dll stdcall';

function GetDriveType(Root: String): Cardinal;
  external 'GetDriveTypeW@kernel32.dll stdcall';

function ReadRegistryValueType(Root: Cardinal; Key, Name: String; Flags: Cardinal;
  var ValueType: Cardinal; Data: Integer; var Size: Cardinal): Integer;
  external 'RegGetValueW@advapi32.dll stdcall';

function OpenRegistryKey(Root: Cardinal; Key: String; Options, Access: Cardinal;
  var Handle: Cardinal): Integer;
  external 'RegOpenKeyExW@advapi32.dll stdcall';

function FlushRegistryKey(Handle: Cardinal): Integer;
  external 'RegFlushKey@advapi32.dll stdcall';

function CloseRegistryKey(Handle: Cardinal): Integer;
  external 'RegCloseKey@advapi32.dll stdcall';

function MissingEntry(Error: Integer): Boolean;
begin
  Result := (Error = MissingFile) or (Error = MissingPath);
end;

function EntryIsMissing(Path: String): Boolean;
begin
  Result := (GetAttributes(Path) = InvalidFileAttributes) and MissingEntry(DLLGetLastError);
end;

function PlainFile(Path: String): Boolean;
var
  Attributes: Cardinal;
begin
  Attributes := GetAttributes(Path);
  Result := (Attributes <> InvalidFileAttributes) and
    (Attributes and (FILE_ATTRIBUTE_DIRECTORY or FILE_ATTRIBUTE_REPARSE_POINT) = 0);
end;

@WEBVIEW2_CODE@

function RecordBytes(Path: String; MaxSize: Integer; var Bytes: AnsiString): Boolean;
var
  Size: Int64;
begin
  Result := PlainFile(Path) and FileSize64(Path, Size) and (Size > 0) and
    (Size <= MaxSize) and LoadStringFromFile(Path, Bytes) and (Length(Bytes) = Size);
end;

function RecordText(Path: String; MaxSize: Integer; var Content: String): Boolean;
var
  Bytes: AnsiString;
begin
  Result := RecordBytes(Path, MaxSize, Bytes);
  if Result then
    Content := String(Bytes);
end;

function RecordMatches(Path, Expected: String): Boolean;
var
  Content: String;
begin
  Result := RecordText(Path, Length(Expected), Content) and (Content = Expected);
end;

function SyncPlainFile(Path: String): Boolean;
var
  Handle: Integer;
begin
  Result := False;
  if not PlainFile(Path) then
    Exit;
  // GENERIC_WRITE, FILE_SHARE_READ, OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT.
  Handle := OpenLeaseFile(Path, $40000000, 1, 0, 3, $00200000, 0);
  if Handle = -1 then
    Exit;
  try
    Result := FlushFile(Handle);
  finally
    CloseKernelHandle(Handle);
  end;
end;

function StageRecord(Path: String; Bytes: AnsiString): String;
begin
  Result := GenerateUniqueName(AddBackslash(ExtractFileDir(Path)), '.tmp');
  if (Result <> '') and (not SaveStringToFile(Result, Bytes, False) or not SyncPlainFile(Result)) then begin
    DeleteFile(Result);
    Result := '';
  end;
end;

function PublishRecord(Path, Content: String): Boolean;
var
  Temporary: String;
begin
  Result := RecordMatches(Path, Content);
  if Result or not EntryIsMissing(Path) then
    Exit;
  Temporary := StageRecord(Path, AnsiString(Content));
  if Temporary = '' then
    Exit;
  try
    Result := PublishFile(Temporary, Path, WriteThrough);
  finally
    if not Result then
      DeleteFile(Temporary);
  end;
end;

function LowerDigest(Value: String): Boolean;
var
  Index: Integer;
begin
  Result := False;
  if Length(Value) <> 64 then
    Exit;
  for Index := 1 to Length(Value) do
    if Pos(Value[Index], '0123456789abcdef') = 0 then
      Exit;
  Result := True;
end;

function PlainVersion(Value: String): Boolean;
var
  Index: Integer;
begin
  Result := False;
  if (Value = '') or (Length(Value) > 128) or (Value[Length(Value)] = '.') or
    (Pos(Value[1], '0123456789') = 0) then
    Exit;
  for Index := 1 to Length(Value) do
    if Pos(Value[Index], '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz.-') = 0 then
      Exit;
  Result := True;
end;

function PlainDirectoryChain(Path: String; MayBeMissing: Boolean): Boolean;
var
  Drive, Parent: String;
  Attributes: Cardinal;
begin
  Result := False;
  Drive := ExtractFileDrive(Path);
  if (Length(Drive) <> 2) or (Copy(Drive, 2, 1) <> ':') then
    Exit;
  if GetDriveType(AddBackslash(Drive)) <> FixedDrive then
    Exit;

  while CompareText(Path, AddBackslash(Drive)) <> 0 do begin
    Attributes := GetAttributes(Path);
    if Attributes = InvalidFileAttributes then begin
      if not MayBeMissing or not MissingEntry(DLLGetLastError) then
        Exit;
    end else if (Attributes and FILE_ATTRIBUTE_DIRECTORY = 0) or
      (Attributes and FILE_ATTRIBUTE_REPARSE_POINT <> 0) then
      Exit;
    Parent := ExtractFileDir(Path);
    if (Parent = '') or (Parent = Path) then
      Exit;
    Path := Parent;
  end;
  Result := True;
end;

function ScopeRootLocator(Machine: Boolean): String;
begin
  if Machine then
    Result := ExpandConstant('{commonpf64}\') + RootLocator
  else
    Result := ExpandConstant('{localappdata}\') + RootLocator;
end;

function ReadScopeRoot(Machine: Boolean; var Root: String): Boolean;
var
  Locator: String;
  Bytes: AnsiString;
  Size: Int64;
  Separator: Integer;
begin
  Result := False;
  Locator := ScopeRootLocator(Machine);
  if not PlainFile(Locator) or not FileSize64(Locator, Size) or (Size < 51) or
    (Size > 4146) or not LoadStringFromFile(Locator, Bytes) or (Length(Bytes) <> Size) then
    Exit;
  Separator := Pos(#10, Bytes);
  if (Separator < 4) or (Length(Bytes) <> Separator + 49) or (Bytes[Length(Bytes)] <> #10) then
    Exit;
  Root := Utf8Decode(Copy(Bytes, 1, Separator - 1));
  Result := (Root <> '') and (Pos('"', Root) = 0) and (Pos(#13, Root) = 0) and
    PlainDirectoryChain(Root, False);
end;

function DefaultProductRoot(Param: String): String;
begin
  if not ReadScopeRoot(IsAdminInstallMode, Result) then
    Result := ExpandConstant('{autopf}\@INSTALL_DIRECTORY@');
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := (PageID = wpSelectDir) and (CommittingUpdate or CompletingBridge or
    not EntryIsMissing(ScopeRootLocator(IsAdminInstallMode)));
end;

function PlainFileMatches(FileName, Digest: String): Boolean;
  forward;

function RootWorkerMatches(Directory: String): Boolean;
begin
  Result := PlainFileMatches(Directory + '\@BOOTSTRAP_FILE@', '@BOOTSTRAP_DIGEST@')@ROOT_WORKER_CHECKS@;
end;

function RunRootWorker(Action, Root: String): Boolean;
var
  Arguments: String;
  ExitCode: Integer;
begin
  Result := False;
  if (Root = '') or (Pos('"', Root) <> 0) or (Pos(#13, Root) <> 0) or (Pos(#10, Root) <> 0) or
    not RootWorkerMatches(InstallRootWorker) then
    Exit;
  Arguments := 'windows-install-root --action ' + Action + ' --root "' + Root + '"';
  if IsAdminInstallMode then
    Arguments := Arguments + ' --machine';
  Result := Exec(InstallRootWorker + '\@BOOTSTRAP_FILE@', Arguments, InstallRootWorker, SW_HIDE,
    ewWaitUntilTerminated, ExitCode) and (ExitCode = 0);
end;

function PrepareRootWorker: Boolean;
begin
  if InstallRootWorker = '' then begin
    ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\@BOOTSTRAP_FILE@');
    ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*.dll');
    InstallRootWorker := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  end;
  Result := RootWorkerMatches(InstallRootWorker);
end;

function MatchesReservedRoot(Root: String): Boolean;
var
  Registered: String;
begin
  Result := ReadScopeRoot(IsAdminInstallMode, Registered) and (CompareText(Root, Registered) = 0);
end;

function CheckPreparedRoot(Root: String): Boolean;
begin
  Result := MatchesReservedRoot(Root) and PrepareRootWorker and RunRootWorker('check', Root);
end;

function ReservePreparedRoot(Root: String): Boolean;
var
  ReservedRoot: String;
begin
  Result := PrepareRootWorker and RunRootWorker('reserve', Root) and CheckPreparedRoot(Root);
  if Result and not IsAdminInstallMode then
    Result := ReadScopeRoot(False, ReservedRoot) and
      RegWriteStringValue(HKEY_CURRENT_USER, UserScopeLocatorKey, 'OwnedRoot', ReservedRoot);
end;

function PreserveUninstallRootWorker(Runtime: String): Boolean;
begin
  Result := False;
  if not RootWorkerMatches(Runtime) then
    Exit;
  InstallRootWorker := ExpandConstant('{tmp}');
  if not FileCopy(Runtime + '\@BOOTSTRAP_FILE@', InstallRootWorker + '\@BOOTSTRAP_FILE@', True) then
    Exit;
@ROOT_WORKER_COPIES@
  Result := RootWorkerMatches(InstallRootWorker);
end;

function EmptyPreparedRoot(Root: String): Boolean;
var
  Entry: TFindRec;
begin
  Result := False;
  if not CheckPreparedRoot(Root) or not FindFirst(AddBackslash(Root) + '*', Entry) then
    Exit;
  try
    repeat
      if (Entry.Name <> '.') and (Entry.Name <> '..') then
        Exit;
    until not FindNext(Entry);
    Result := True;
  finally
    FindClose(Entry);
  end;
end;

function ConflictWithLoadedProfile: Boolean;
var
  Profiles: TArrayOfString;
  Index: Integer;
begin
  Result := True;
  if not RegGetSubkeyNames(HKEY_USERS, '', Profiles) then
    Exit;
  for Index := 0 to GetArrayLength(Profiles) - 1 do
    if RegKeyExists(HKEY_USERS, Profiles[Index] + '\' + UninstallKey) or
      RegValueExists(HKEY_USERS, Profiles[Index] + '\' + UserScopeLocatorKey, 'OwnedRoot') then
      Exit;
  Result := False;
end;

function MachineInstallationPresent: Boolean;
begin
  // An interrupted update can have deleted the machine uninstall key without
  // removing its installation. A different user must still see the conflict.
  Result := RegKeyExists(HKEY_LOCAL_MACHINE_64, UninstallKey) or
    not EntryIsMissing(ScopeRootLocator(True)) or
    not EntryIsMissing(ExpandConstant('{commonpf}\@INSTALL_DIRECTORY@'));
end;

function PrepareUpdate(Root: String; var NeedsRestart: Boolean; var PrerequisiteError: String): Boolean;
  forward;

function PrepareRepair(Root: String): Boolean;
  forward;

function ApplyRepair(Root: String): Boolean;
  forward;

function PrepareFreshBridge(Root: String; var NeedsRestart: Boolean): String;
  forward;

function PrepareScopeConversion(Root: String; var NeedsRestart: Boolean): String;
  forward;

function InstallRegistryRoot: Cardinal;
  forward;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Root, PrerequisiteError: String;
  ScopeReserved: Boolean;
begin
  Result := 'Use an ALCOMD installation folder on a local fixed drive with a protected parent.';
  Root := RemoveBackslashUnlessRoot(WizardDirValue);
  if not PlainDirectoryChain(ExtractFileDir(Root), True) or (Pos('"', Root) <> 0) then
    Exit;
  ScopeReserved := not EntryIsMissing(ScopeRootLocator(IsAdminInstallMode));
  if not ScopeReserved and (CompareText(Root, ExpandConstant('{autopf}\@INSTALL_DIRECTORY@')) <> 0) and
    not EntryIsMissing(ExpandConstant('{autopf}\@INSTALL_DIRECTORY@')) then begin
    Result := 'An existing folder at the default ALCOMD location must be verified before selecting another root.';
    Exit;
  end;
  if ScopeReserved and not CheckPreparedRoot(Root) then begin
    Result := 'The reserved ALCOMD installation root could not be verified.';
    Exit;
  end;

  if CommittingUpdate then begin
    Result := 'The ALCOMD update could not verify the existing installation or its recovery records.';
    if ScopeReserved and PrepareUpdate(Root, NeedsRestart, PrerequisiteError) then
      Result := ''
    else if PrerequisiteError <> '' then
      Result := PrerequisiteError;
    Exit;
  end;
  if CompletingBridge then begin
    Result := PrepareFreshBridge(Root, NeedsRestart);
    Exit;
  end;

  if (IsAdminInstallMode and ConflictWithLoadedProfile) or
    (not IsAdminInstallMode and MachineInstallationPresent) then begin
    Result := PrepareScopeConversion(Root, NeedsRestart);
    Exit;
  end;

  Result := 'The destination already exists or could not be checked. Its contents were kept.';
  if not EntryIsMissing(Root) and not (ScopeReserved and EmptyPreparedRoot(Root)) then begin
    Repairing := PrepareRepair(Root);
    if Repairing then begin
      Result := EnsureWebView2(NeedsRestart);
      if (Result = '') and not ApplyRepair(Root) then
        Result := 'ALCOMD could not restore all missing program files. Retry with the same installer.';
    end;
    Exit;
  end;
  if not EntryIsMissing(AddBackslash(ExtractFileDir(Root)) +
    '.@INSTALL_DIRECTORY@@FRESH_STAGE_PREFIX@@MANIFEST_DIGEST@') then begin
    Result := 'An ALCOMD migration is staged. Resume it with the migration bridge.';
    Exit;
  end;

  Result := 'An existing ALCOMD installation was found or could not be ruled out.';
  if (not IsAdminInstallMode and MachineInstallationPresent) or
    RegKeyExists(InstallRegistryRoot, UninstallKey) then
    Exit;
  if IsAdminInstallMode and ConflictWithLoadedProfile then
    Exit;
  Result := EnsureWebView2(NeedsRestart);
  if (Result = '') and not ReservePreparedRoot(Root) then
    Result := 'The ALCOMD installation directory could not be reserved safely.';
end;

function OwnedUninstallRoot(Root: String): Boolean;
var
  RegisteredRoot: String;
begin
  Result := False;
  if not MatchesReservedRoot(Root) then
    Exit;
  if IsAdminInstallMode then begin
    if not RegQueryStringValue(HKEY_LOCAL_MACHINE_64, UninstallKey, 'InstallLocation', RegisteredRoot) then
      Exit;
  end else begin
    if not RegQueryStringValue(HKEY_CURRENT_USER, UninstallKey, 'InstallLocation', RegisteredRoot) then
      Exit;
  end;
  Result := CompareText(Root, RemoveBackslashUnlessRoot(RegisteredRoot)) = 0;
end;

function InstallRegistryRoot: Cardinal;
begin
  if IsAdminInstallMode then
    Result := HKEY_LOCAL_MACHINE_64
  else
    Result := HKEY_CURRENT_USER;
end;

@PROTOCOL_CODE@

function UpdateScopeConflicts: Boolean;
begin
  if IsAdminInstallMode then
    Result := ConflictWithLoadedProfile
  else
    Result := MachineInstallationPresent;
end;

function PublishStagedRuntime(Source, Destination: String): Boolean;
  forward;

function InitializeSetup: Boolean;
var
  Digest, FreshDigest, FinishDigest, Commit, Scope, Root, Parent, Prefix, Destination, Extracted: String;
  PrerequisiteError: String;
  NeedsRestart: Boolean;
begin
  Digest := ExpandConstant('{param:ALCOMDStage|}');
  FreshDigest := ExpandConstant('{param:ALCOMDStageFresh|}');
  FinishDigest := ExpandConstant('{param:ALCOMDFinishFresh|}');
  BridgeJournal := ExpandConstant('{param:ALCOMDBridgeJournal|}');
  Commit := ExpandConstant('{param:ALCOMDCommit|}');
  Scope := ExpandConstant('{param:ALCOMDScope|}');
  PreviousVersion := ExpandConstant('{param:ALCOMDPrevious|}');
  PreviousDigest := ExpandConstant('{param:ALCOMDPreviousDigest|}');
  CommittingUpdate := Commit <> '';
  CompletingBridge := FinishDigest <> '';
  Result := (Digest = '') and (FreshDigest = '') and (FinishDigest = '') and
    (BridgeJournal = '') and (Commit = '') and (Scope = '') and
    (PreviousVersion = '') and (PreviousDigest = '');
  if Result then
    Exit;
  // Bind private update modes to the requested scope before accessing either installation.
  if ((Scope = '') and IsAdminInstallMode) or
    ((Scope <> '') and ((Scope <> 'all-users') or not IsAdminInstallMode)) then
    Exit;
  if CommittingUpdate then begin
    Result := (Digest = '') and (FreshDigest = '') and (FinishDigest = '') and
      (BridgeJournal = '') and
      (Commit = '@MANIFEST_DIGEST@') and PlainVersion(PreviousVersion) and
      (PreviousVersion <> '@APP_VERSION@') and LowerDigest(PreviousDigest);
    Exit;
  end;
  if CompletingBridge then begin
    Result := (Digest = '') and (FreshDigest = '') and (FinishDigest = '@MANIFEST_DIGEST@') and
      (BridgeJournal <> '') and (Pos('"', BridgeJournal) = 0) and
      (PreviousVersion = '') and (PreviousDigest = '');
    Exit;
  end;
  Root := DefaultProductRoot('');
  Parent := ExtractFileDir(Root);
  if (BridgeJournal <> '') or (PreviousVersion <> '') or (PreviousDigest <> '') or UpdateScopeConflicts then
    Exit;
  if FreshDigest <> '' then begin
    // The migration bridge checks the complete installer before requesting an
    // inactive sibling copy. This path creates neither an install nor a selector.
    if (Digest <> '') or (FreshDigest <> '@MANIFEST_DIGEST@') or
      not EntryIsMissing(Root) or not EntryIsMissing(ScopeRootLocator(IsAdminInstallMode)) or
      RegKeyExists(InstallRegistryRoot, UninstallKey) or
      not PlainDirectoryChain(Parent, True) or not ForceDirectories(Parent) or
      not PlainDirectoryChain(Parent, False) then
      Exit;
    PrerequisiteError := EnsureWebView2(NeedsRestart);
    if PrerequisiteError <> '' then begin
      Log(PrerequisiteError);
      Exit;
    end;
    Prefix := '@FRESH_STAGE_PREFIX@';
    Digest := FreshDigest;
  end else begin
    if (Digest <> '@MANIFEST_DIGEST@') or not CheckPreparedRoot(Root) or not OwnedUninstallRoot(Root) or
      not PlainDirectoryChain(Root, False) then
      Exit;
    Prefix := '@UPDATE_STAGE_PREFIX@';
  end;
  // Both success and failure leave the normal Inno installation path unused.
  Destination := AddBackslash(Parent) + '.@INSTALL_DIRECTORY@' + Prefix + Digest;
  if not EntryIsMissing(Destination) then
    Exit;
  ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*');
  Extracted := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  if not PublishStagedRuntime(Extracted, Destination) then
    Log('Could not publish the ALCOMD staging directory.');
end;

function IsFreshInstallation: Boolean;
begin
  Result := not CommittingUpdate and not Repairing;
end;

function IsRepairInstallation: Boolean;
begin
  Result := Repairing and not RebuildUninstallRegistration;
end;

function IsInitialActivation: Boolean;
begin
  Result := IsFreshInstallation and not ConvertingScope and not CompletingBridge;
end;

function IsUpdateInstallation: Boolean;
begin
  Result := CommittingUpdate or ConvertingScope or CompletingBridge or RebuildUninstallRegistration;
end;

function IsBridgeInstallation: Boolean;
begin
  Result := CompletingBridge or RebuildUninstallRegistration;
end;

function CreateFreshIcon: Boolean;
begin
  Result := not CommittingUpdate and not Repairing and not WizardNoIcons;
end;

function CreateUpdateIcon: Boolean;
begin
  Result := CommittingUpdate and LowerDigest(PreviousShortcut) and
    EntryIsMissing(UpdateJournal + '\' + CandidateShortcutFile);
end;

function ReadPath(var Path: String; var ValueType: Cardinal): Boolean;
var
  Key: String;
  Root: Cardinal;
  Size: Cardinal;
  ReadResult: Integer;
begin
  if IsAdminInstallMode then begin
    Root := HKEY_LOCAL_MACHINE;
    Key := MachineEnvironment;
  end else begin
    Root := HKEY_CURRENT_USER;
    Key := UserEnvironment;
  end;
  Size := 0;
  ValueType := 0;
  Path := '';
  ReadResult := ReadRegistryValueType(Root, Key, 'Path', ReadEnvironmentString, ValueType, 0, Size);
  Result := ReadResult = MissingFile;
  if Result then begin
    ValueType := 0;
    Exit;
  end;
  if (ReadResult <> 0) or (Size > 65534) then
    Exit;
  Result := RegQueryStringValue(InstallRegistryRoot, Key, 'Path', Path);
end;

function WritePath(Path: String; ValueType: Cardinal): Boolean;
var
  Key: String;
  Root: Cardinal;
begin
  Root := InstallRegistryRoot;
  if IsAdminInstallMode then
    Key := MachineEnvironment
  else
    Key := UserEnvironment;
  case ValueType of
    0: Result := RegDeleteValue(Root, Key, 'Path');
    StringValue: Result := RegWriteStringValue(Root, Key, 'Path', Path);
    ExpandStringValue: Result := RegWriteExpandStringValue(Root, Key, 'Path', Path);
  else
    Result := False;
  end;
end;

function PathEntryIsPresent(Path, Entry: String): Boolean;
var
  Delimiter: Integer;
  Candidate: String;
begin
  Path := Path + ';';
  while Path <> '' do begin
    Delimiter := Pos(';', Path);
    Candidate := Trim(Copy(Path, 1, Delimiter - 1));
    Delete(Path, 1, Delimiter);
    if (Length(Candidate) >= 2) and (Candidate[1] = '"') and
      (Candidate[Length(Candidate)] = '"') then
      Candidate := Copy(Candidate, 2, Length(Candidate) - 2);
    if CompareText(RemoveBackslashUnlessRoot(Candidate), Entry) = 0 then begin
      Result := True;
      Exit;
    end;
  end;
  Result := False;
end;

function ExactPathSlot(Path, Entry: String): Integer;
begin
  Result := Pos(Entry, Path);
  if (Result <> 0) and
    (Pos(Entry, Copy(Path, Result + Length(Entry) - 1, Length(Path))) <> 0) then
    Result := 0;
end;

function ReadShellReceipt(Path: String; var Claim: String): Boolean;
var
  Bytes: AnsiString;
begin
  // A protected scope can be installed at a Unicode path.
  Result := RecordBytes(Path, 131072, Bytes);
  if not Result then
    Exit;
  Claim := Utf8Decode(Bytes);
  Result := Utf8Encode(Claim) = Bytes;
end;

function ShellReceiptMatches(Path, Claim: String): Boolean;
var
  Recorded: String;
begin
  Result := ReadShellReceipt(Path, Recorded) and (Recorded = Claim);
end;

function WriteShellReceipt(Path, Claim, Previous: String): Boolean;
var
  Temporary: String;
begin
  Result := ShellReceiptMatches(Path, Claim);
  if Result then
    Exit;
  Temporary := StageRecord(Path, Utf8Encode(Claim));
  if Temporary = '' then
    Exit;
  try
    if Previous = '' then
      Result := EntryIsMissing(Path) and PublishFile(Temporary, Path, WriteThrough)
    else
      Result := ShellReceiptMatches(Path, Previous) and
        PublishFile(Temporary, Path, ReplaceExisting or WriteThrough);
  finally
    if not Result then
      DeleteFile(Temporary);
  end;
end;

function ClearShellReceipt(Path, Claim: String): Boolean;
begin
  Result := ShellReceiptMatches(Path, Claim) and DeleteFile(Path);
end;

function InstallCliPath: Boolean;
var
  Entry, Path, Claim, Receipt: String;
  ValueType: Cardinal;
begin
  Result := False;
  Entry := ExpandConstant('{app}\bin');
  Receipt := ExpandConstant('{app}\') + CliPathReceipt;
  if (Pos(';', Entry) <> 0) or (Pos('%', Entry) <> 0) or not ReadPath(Path, ValueType) then
    Exit;
  if PathEntryIsPresent(Path, Entry) then begin
    Result := True;
    Exit;
  end;
  Claim := 'existing|' + Entry;
  if ValueType = 0 then begin
    Claim := 'missing|' + Entry;
    ValueType := ExpandStringValue;
  end;
  if Path <> '' then
    Path := Path + ';';
  Path := Path + Entry;
  if Length(Path) > 32766 then
    Exit;
  if not (CompletingBridge or EntryIsMissing(Receipt)) or not WriteShellReceipt(Receipt, Claim, '') then
    Exit;
  Result := WritePath(Path, ValueType);
  if not Result then
    if not ClearShellReceipt(Receipt, Claim) then
      Log('Could not clear the CLI PATH ownership receipt.');
end;

function RemoveOwnedCliPath: Boolean;
var
  Entry, Path, Claim, Token, Receipt: String;
  ValueType: Cardinal;
  Position: Integer;
begin
  Result := True;
  Receipt := ExpandConstant('{app}\') + CliPathReceipt;
  if not ReadShellReceipt(Receipt, Claim) then
    Exit;
  Entry := ExpandConstant('{app}\bin');
  if (Claim <> 'missing|' + Entry) and (Claim <> 'existing|' + Entry) then
    Exit;
  if not ReadPath(Path, ValueType) then begin
    Result := False;
    Exit;
  end;
  Path := ';' + Path + ';';
  Token := ';' + Entry + ';';
  Position := ExactPathSlot(Path, Token);
  if Position <> 0 then begin
    Delete(Path, Position, Length(Token) - 1);
    if Length(Path) > 1 then
      Path := Copy(Path, 2, Length(Path) - 2)
    else
      Path := '';
    if (Path = '') and (Claim = 'missing|' + Entry) then
      ValueType := 0;
    if not WritePath(Path, ValueType) then begin
      Result := False;
      Exit;
    end;
  end;
  Result := ClearShellReceipt(Receipt, Claim);
end;

function ReadDigest(FileName: String; var Digest: String): Boolean;
begin
  Result := False;
  if not PlainFile(FileName) then
    Exit;
  try
    Digest := Lowercase(GetSHA256OfFile(FileName));
    Result := True;
  except
    Log('Could not read an installed ALCOMD file.');
  end;
end;

function PlainFileMatches(FileName, Digest: String): Boolean;
var
  Observed: String;
begin
  Result := ReadDigest(FileName, Observed) and (Observed = Digest);
end;

function StartMenuEntry: String;
begin
  Result := ExpandConstant('{autoprograms}\@APP_NAME@.lnk');
end;

function PublishCopy(Source, Destination, Digest, Previous, Scratch: String): Boolean;
var
  Temporary: String;
  Flags: Cardinal;
begin
  Result := PlainFileMatches(Destination, Digest);
  if Result then
    Exit;
  if not PlainFileMatches(Source, Digest) then
    Exit;
  if Previous = '' then
    Flags := WriteThrough
  else
    Flags := WriteThrough or ReplaceExisting;
  Temporary := GenerateUniqueName(AddBackslash(Scratch), '.tmp');
  if (Temporary = '') or not CopyFile(Source, Temporary, True) then
    Exit;
  try
    if not PlainFileMatches(Temporary, Digest) or not SyncPlainFile(Temporary) then
      Exit;
    if Previous = '' then begin
      if not EntryIsMissing(Destination) then
        Exit;
    end else if not PlainFileMatches(Destination, Previous) then
      Exit;
    Result := PublishFile(Temporary, Destination, Flags);
  finally
    if not Result then
      DeleteFile(Temporary);
  end;
end;

function ReadStartMenuReceipt(Path, Destination: String; var Digest: String): Boolean;
var
  Claim: String;
begin
  Result := False;
  if not ReadShellReceipt(Path, Claim) or (Copy(Claim, 65, Length(Claim)) <> '|' + Destination) then
    Exit;
  Digest := Copy(Claim, 1, 64);
  Result := LowerDigest(Digest);
end;

function SnapshotStartMenu(Root: String): Boolean;
var
  Shortcut, Digest: String;
begin
  Result := False;
  PreviousShortcut := 'none';
  Shortcut := StartMenuEntry;
  if ReadStartMenuReceipt(Root + '\' + StartMenuReceipt, Shortcut, Digest) and
    PlainDirectoryChain(ExtractFileDir(Shortcut), False) and PlainFileMatches(Shortcut, Digest) then begin
    if not PublishCopy(Shortcut, UpdateJournal + '\' + PreviousShortcutFile, Digest, '', UpdateJournal) then
      Exit;
    PreviousShortcut := Digest;
  end;
  Result := PublishRecord(UpdateJournal + '\' + ShortcutSnapshot, PreviousShortcut + #10);
end;

function ReadShortcutSnapshot: Boolean;
var
  Content: String;
begin
  Result := False;
  if not RecordText(UpdateJournal + '\' + ShortcutSnapshot, 65, Content) then
    Exit;
  if Content[Length(Content)] <> #10 then
    Exit;
  PreviousShortcut := Copy(Content, 1, Length(Content) - 1);
  Result := (PreviousShortcut = 'none') or (LowerDigest(PreviousShortcut) and
    PlainFileMatches(UpdateJournal + '\' + PreviousShortcutFile, PreviousShortcut));
end;

function UpdateStartMenuEntry: Boolean;
var
  Candidate, Destination, Digest, Extracted, Receipt, PreviousClaim: String;
begin
  Result := True;
  if PreviousShortcut = 'none' then
    Exit;
  Destination := StartMenuEntry;
  if not PlainDirectoryChain(ExtractFileDir(Destination), False) then
    Exit;
  Receipt := ExpandConstant('{app}\') + StartMenuReceipt;
  PreviousClaim := PreviousShortcut + '|' + Destination;
  Candidate := UpdateJournal + '\' + CandidateShortcutFile;
  if EntryIsMissing(Candidate) then begin
    // Inno produces this link with the permanent AUMID. Keep its exact bytes for retries.
    Extracted := ExpandConstant('{tmp}\') + CandidateShortcutFile;
    Result := ReadDigest(Extracted, Digest) and PublishCopy(Extracted, Candidate, Digest, '', UpdateJournal);
  end else
    Result := ReadDigest(Candidate, Digest);
  if not Result then
    Exit;
  if ShellReceiptMatches(Receipt, PreviousClaim) and PlainFileMatches(Destination, PreviousShortcut) then begin
    Result := PublishCopy(Candidate, Destination, Digest, PreviousShortcut, ExtractFileDir(Destination));
    if not Result then
      Exit;
  end;
  // If the user replaced or removed the prior link, preserve their choice.
  if PlainFileMatches(Destination, Digest) and ShellReceiptMatches(Receipt, PreviousClaim) then
    Result := WriteShellReceipt(Receipt, Digest + '|' + Destination, PreviousClaim);
end;

function UpdateLeaseIsHeld(Manifest: String): Boolean;
var
  Handle: Integer;
  Range: TLeaseRange;
begin
  Result := False;
  Handle := OpenLeaseFile(Manifest, $80000000, 5, 0, 3, $00200000, 0);
  if Handle = -1 then
    Exit;
  Range.Internal := 0;
  Range.InternalHigh := 0;
  Range.Offset := 0;
  Range.OffsetHigh := 1;
  Range.EventHandle := 0;
  try
    // FAIL_IMMEDIATELY without EXCLUSIVE: only the Bootstrap maintenance lease blocks this probe.
    if not TryLockLease(Handle, 1, 0, 1, 0, Range) then
      Result := DLLGetLastError = LockViolation;
  finally
    CloseKernelHandle(Handle);
  end;
end;

function UpdateRegistryMatches(Root: String; Resuming: Boolean): Boolean;
var
  Registered, Version: String;
  Names: TArrayOfString;
  Index: Integer;
  RegistryRoot: Cardinal;
begin
  Result := False;
  if UpdateScopeConflicts then
    Exit;
  RegistryRoot := InstallRegistryRoot;
  if OwnedUninstallRoot(Root) then begin
    if RegQueryStringValue(RegistryRoot, UninstallKey, 'DisplayVersion', Version) then
      Result := (Version = PreviousVersion) or (Resuming and (Version = '@APP_VERSION@'))
    else
      Result := Resuming and not RegValueExists(RegistryRoot, UninstallKey, 'DisplayVersion');
    Exit;
  end;
  if not Resuming then
    Exit;
  // Stock Inno recreates the uninstall key. A retry can encounter its initial, incomplete prefix.
  if RegValueExists(RegistryRoot, UninstallKey, 'InstallLocation') then
    Exit;
  if not RegKeyExists(RegistryRoot, UninstallKey) then begin
    Result := True;
    Exit;
  end;
  if not RegGetValueNames(RegistryRoot, UninstallKey, Names) then
    Exit;
  for Index := 0 to GetArrayLength(Names) - 1 do
    if (Names[Index] <> 'Inno Setup: Setup Version') and (Names[Index] <> 'Inno Setup: App Path') then
      Exit;
  if RegValueExists(RegistryRoot, UninstallKey, 'Inno Setup: App Path') then
    if not RegQueryStringValue(RegistryRoot, UninstallKey, 'Inno Setup: App Path', Registered) or
      (CompareText(Registered, Root) <> 0) then
      Exit;
  Result := True;
end;

function CopyRuntimeFile(Directory, Relative: String): Boolean;
var
  Destination, Parent, Digest: String;
begin
  Result := False;
  Destination := Directory + '\' + Relative;
  Parent := ExtractFileDir(Destination);
  if not PlainDirectoryChain(Parent, True) or not ForceDirectories(Parent) or
    not PlainDirectoryChain(Parent, False) or not ReadDigest(UpdateStage + '\' + Relative, Digest) then
    Exit;
  Result := PublishCopy(UpdateStage + '\' + Relative, Destination, Digest, '', UpdateJournal);
end;

function CopyRuntime(Directory: String): Boolean;
begin
  Result := False;
@UPDATE_FILES@
  Result := True;
end;

function RuntimeMatchesStage(Directory, Stage: String; Depth: Integer; var Remaining: Integer): Boolean;
var
  Entry: TFindRec;
  Child, Reference, Digest: String;
begin
  Result := False;
  if (Depth > 4) or not PlainDirectoryChain(Directory, False) or not PlainDirectoryChain(Stage, False) or
    not FindFirst(AddBackslash(Directory) + '*', Entry) then
    Exit;
  try
    repeat
      if (Entry.Name <> '.') and (Entry.Name <> '..') then begin
        if (Entry.Attributes and FILE_ATTRIBUTE_REPARSE_POINT <> 0) or (Remaining = 0) then
          Exit;
        Child := AddBackslash(Directory) + Entry.Name;
        Reference := AddBackslash(Stage) + Entry.Name;
        if Entry.Attributes and FILE_ATTRIBUTE_DIRECTORY <> 0 then begin
          if not RuntimeMatchesStage(Child, Reference, Depth + 1, Remaining) then
            Exit;
        end else begin
          if not ReadDigest(Reference, Digest) or not PlainFileMatches(Child, Digest) then
            Exit;
          Remaining := Remaining - 1;
        end;
      end;
    until not FindNext(Entry);
    Result := True;
  finally
    FindClose(Entry);
  end;
end;

function PublishStagedRuntime(Source, Destination: String): Boolean;
var
  Scratch, Runtime: String;
  Remaining: Integer;
begin
  Result := False;
  if not PlainDirectoryChain(Source, False) or
    not PlainFileMatches(Source + '\@MANIFEST_NAME@', '@MANIFEST_DIGEST@') then
    Exit;
  // Create fresh files on the destination volume so they inherit the installation
  // parent's ACL, including when a different administrator later resumes the update.
  Scratch := GenerateUniqueName(AddBackslash(ExtractFileDir(Destination)), '.tmp');
  if (Scratch = '') or not CreateDir(Scratch) then
    Exit;
  try
    UpdateStage := Source;
    UpdateJournal := Scratch;
    Runtime := Scratch + '\runtime';
    Remaining := @RUNTIME_FILE_COUNT@;
    Result := CopyRuntime(Runtime) and RuntimeMatchesStage(Runtime, Source, 0, Remaining) and
      (Remaining = 0) and EntryIsMissing(Destination) and PublishFile(Runtime, Destination, WriteThrough);
  finally
    if not DelTree(Scratch, True, True, True) then
      Log('Could not remove the ALCOMD update extraction scratch directory.');
  end;
end;

function StableBinOwned(Root, OldRuntime, Name: String): Boolean;
var
  InstalledDigest, Prior, Candidate: String;
begin
  Result := ReadDigest(Root + '\bin\' + Name, InstalledDigest) and
    ReadDigest(OldRuntime + '\' + Name, Prior) and
    ReadDigest(UpdateStage + '\' + Name, Candidate) and
    ((InstalledDigest = Prior) or (InstalledDigest = Candidate));
end;

function PublishStableBin(Root, OldRuntime, Name: String): Boolean;
var
  Prior, Candidate: String;
begin
  Result := ReadDigest(OldRuntime + '\' + Name, Prior) and
    ReadDigest(UpdateStage + '\' + Name, Candidate) and
    PublishCopy(UpdateStage + '\' + Name, Root + '\bin\' + Name, Candidate, Prior, UpdateJournal);
end;

function PrepareUpdate(Root: String; var NeedsRestart: Boolean; var PrerequisiteError: String): Boolean;
var
  OldRuntime, NewRuntime, CandidateRuntime, Manifest, Intent: String;
  Resuming: Boolean;
  Remaining: Integer;
begin
  Result := False;
  if not PlainDirectoryChain(Root + '\bin', False) or
    not EntryIsMissing(Root + '\' + ScopeTransition) then
    Exit;
  Intent := PreviousVersion + '|' + PreviousDigest + '|@APP_VERSION@|@MANIFEST_DIGEST@' + #10;
  if not RecordMatches(Root + '\' + UpdateIntent, Intent) then
    Exit;
  UpdateStage := AddBackslash(ExtractFileDir(Root)) +
    '.@INSTALL_DIRECTORY@@UPDATE_STAGE_PREFIX@@MANIFEST_DIGEST@';
  UpdateJournal := UpdateStage + '.transaction';
  if not PlainDirectoryChain(UpdateStage, False) or not PlainDirectoryChain(UpdateJournal, False) or
    not EntryIsMissing(UpdateJournal + '\' + UpdateInstalled) or
    not PlainFileMatches(UpdateStage + '\@MANIFEST_NAME@', '@MANIFEST_DIGEST@') then
    Exit;
  Resuming := not EntryIsMissing(UpdateJournal + '\' + ShortcutSnapshot);
  if not RecordMatches(Root + '\' + ActiveRuntime, PreviousVersion + #10) and
    not (Resuming and RecordMatches(Root + '\' + ActiveRuntime, '@APP_VERSION@' + #10)) then
    Exit;
  OldRuntime := Root + '\runtime\' + PreviousVersion;
  Manifest := OldRuntime + '\@MANIFEST_NAME@';
  if not PlainDirectoryChain(OldRuntime, False) or not PlainFileMatches(Manifest, PreviousDigest) or
    not UpdateLeaseIsHeld(Manifest) or not UpdateRegistryMatches(Root, Resuming) or
    not StableBinOwned(Root, OldRuntime, '@CLI_FILE@') or
    not StableBinOwned(Root, OldRuntime, '@CRT_FILE@') then
    Exit;
  PrerequisiteError := EnsureWebView2(NeedsRestart);
  if PrerequisiteError <> '' then
    Exit;
  if Resuming then begin
    if not ReadShortcutSnapshot then
      Exit;
  end else if not SnapshotStartMenu(Root) then
    Exit;

  // All copy work lives in the transaction until a complete, signed runtime can be renamed.
  // Windows may leave temporary files on process exit, so they never enter the runtime tree.
  NewRuntime := Root + '\runtime\@APP_VERSION@';
  if EntryIsMissing(NewRuntime) then begin
    CandidateRuntime := UpdateJournal + '\runtime';
    Remaining := @RUNTIME_FILE_COUNT@;
    if not CopyRuntime(CandidateRuntime) or
      not RuntimeMatchesStage(CandidateRuntime, UpdateStage, 0, Remaining) or (Remaining <> 0) or
      not PublishFile(CandidateRuntime, NewRuntime, WriteThrough) then
      Exit;
  end else begin
    Remaining := @RUNTIME_FILE_COUNT@;
    if not RuntimeMatchesStage(NewRuntime, UpdateStage, 0, Remaining) or (Remaining <> 0) then
      Exit;
  end;
  Result := PublishStableBin(Root, OldRuntime, '@CLI_FILE@') and PublishStableBin(Root, OldRuntime, '@CRT_FILE@');
end;

function AcquireProgramLease(Manifest: String): Boolean;
  forward;

function RepairFilesMatch(Root: String; Complete: Boolean): Boolean;
var
  Runtime, Cli, Crt, Digest: String;
  Remaining: Integer;
begin
  Result := False;
  Runtime := Root + '\runtime\@APP_VERSION@';
  Cli := Root + '\bin\@CLI_FILE@';
  Crt := Root + '\bin\@CRT_FILE@';
  Remaining := @RUNTIME_FILE_COUNT@;
  if not PlainDirectoryChain(Root + '\bin', not Complete) or
    not RuntimeMatchesStage(Runtime, RepairStage, 0, Remaining) or
    (Complete and (Remaining <> 0)) or
    not ReadDigest(RepairStage + '\@CLI_FILE@', Digest) or
    not (PlainFileMatches(Cli, Digest) or (not Complete and EntryIsMissing(Cli))) then
    Exit;
  Result := PlainFileMatches(Crt, '@CRT_DIGEST@') or (not Complete and EntryIsMissing(Crt));
end;

function PrepareRepair(Root: String): Boolean;
var
  Runtime, Manifest, InstalledVersion: String;
begin
  Result := False;
  Runtime := Root + '\runtime\@APP_VERSION@';
  Manifest := Runtime + '\@MANIFEST_NAME@';
  if not CheckPreparedRoot(Root) or UpdateScopeConflicts or not PlainDirectoryChain(Runtime, False) or
    not EntryIsMissing(Root + '\' + UpdateIntent) or
    not EntryIsMissing(Root + '\' + ScopeTransition) or
    not RecordMatches(Root + '\' + ActiveRuntime, '@APP_VERSION@' + #10) or
    not PlainFileMatches(Manifest, '@MANIFEST_DIGEST@') then
    Exit;
  RebuildUninstallRegistration := not RegKeyExists(InstallRegistryRoot, UninstallKey);
  if not RebuildUninstallRegistration then
    if not OwnedUninstallRoot(Root) or
      not RegQueryStringValue(InstallRegistryRoot, UninstallKey, 'DisplayVersion', InstalledVersion) or
      (InstalledVersion <> '@APP_VERSION@') then
      Exit;
  if ProgramLease <= 0 then
    if not AcquireProgramLease(Manifest) then
      Exit;
  if RepairStage = '' then begin
    ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*');
    RepairStage := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  end;
  Result := RepairFilesMatch(Root, False);
end;

function ApplyRepair(Root: String): Boolean;
var
  Scratch: String;
begin
  Result := False;
  Scratch := GenerateUniqueName(AddBackslash(Root), '.tmp');
  if (Scratch = '') or not CreateDir(Scratch) then
    Exit;
  try
    UpdateStage := RepairStage;
    UpdateJournal := Scratch;
    Result := CopyRuntime(Root + '\runtime\@APP_VERSION@') and
      CopyRuntimeFile(Root + '\bin', '@CLI_FILE@') and
      CopyRuntimeFile(Root + '\bin', '@CRT_FILE@') and
      RepairFilesMatch(Root, True);
  finally
    if not RemoveDir(Scratch) then
      Log('Could not remove the ALCOMD repair scratch directory.');
  end;
end;

function BridgeRegistrationMatches(Root: String): Boolean;
var
  RegistryRoot: Cardinal;
  Registered, Version: String;
  Names: TArrayOfString;
  Index: Integer;
begin
  Result := False;
  RegistryRoot := InstallRegistryRoot;
  if not RegKeyExists(RegistryRoot, UninstallKey) then begin
    Result := True;
    Exit;
  end;
  if not RegGetSubkeyNames(RegistryRoot, UninstallKey, Names) or (GetArrayLength(Names) <> 0) then
    Exit;
  // The installer writes App Path before InstallLocation and DisplayVersion.
  if not RegValueExists(RegistryRoot, UninstallKey, 'Inno Setup: App Path') then begin
    if not RegGetValueNames(RegistryRoot, UninstallKey, Names) then
      Exit;
    for Index := 0 to GetArrayLength(Names) - 1 do
      if Names[Index] <> 'Inno Setup: Setup Version' then
        Exit;
    Result := True;
    Exit;
  end;
  if not RegQueryStringValue(RegistryRoot, UninstallKey, 'Inno Setup: App Path', Registered) or
    (CompareText(Registered, Root) <> 0) then
    Exit;
  if RegValueExists(RegistryRoot, UninstallKey, 'InstallLocation') and not OwnedUninstallRoot(Root) then
    Exit;
  if RegValueExists(RegistryRoot, UninstallKey, 'DisplayVersion') then
    if not RegQueryStringValue(RegistryRoot, UninstallKey, 'DisplayVersion', Version) or
      (Version <> '@APP_VERSION@') then
      Exit;
  Result := True;
end;

function BridgeRootIsPlain(Root: String): Boolean;
var
  Entry: TFindRec;
begin
  Result := False;
  if not PlainDirectoryChain(Root, False) or not FindFirst(AddBackslash(Root) + '*', Entry) then
    Exit;
  try
    repeat
      if (Entry.Name <> '.') and (Entry.Name <> '..') and
        (Entry.Attributes and FILE_ATTRIBUTE_REPARSE_POINT <> 0) then
        Exit;
    until not FindNext(Entry);
    Result := True;
  finally
    FindClose(Entry);
  end;
end;

function PrepareFreshBridge(Root: String; var NeedsRestart: Boolean): String;
var
  Worker, Arguments, Scope, Runtime, Bin: String;
  ExitCode, Remaining: Integer;
  Verified: Boolean;
begin
  Result := 'The ALCOMD migration or staged installation could not be verified.';
  if UpdateScopeConflicts then
    Exit;
  BridgeStage := AddBackslash(ExtractFileDir(Root)) + '.@INSTALL_DIRECTORY@@FRESH_STAGE_PREFIX@@MANIFEST_DIGEST@';
  if BridgeExtracted = '' then begin
    ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*');
    BridgeExtracted := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  end;
  Remaining := @RUNTIME_FILE_COUNT@;
  Worker := BridgeStage + '\@BOOTSTRAP_FILE@';
  if not RuntimeMatchesStage(BridgeStage, BridgeExtracted, 0, Remaining) or (Remaining <> 0) or
    not PlainFileMatches(Worker, '@BOOTSTRAP_DIGEST@') then
    Exit;
  Scope := 'current-user';
  if IsAdminInstallMode then
    Scope := 'all-users';
  Arguments := 'verify-windows-bridge-commit --scope ' + Scope +
    ' --manifest-sha256 @MANIFEST_DIGEST@ --work-directory "' + BridgeJournal + '"';
  Verified := ExecAsOriginalUser(Worker, Arguments, BridgeStage, SW_HIDE, ewWaitUntilTerminated, ExitCode);
  if not Verified or (ExitCode <> 0) then
    Exit;

  Result := 'The prepared migration installation could not be resumed safely.';
  BridgeIntent := BridgeStage + '.commit';
  BridgeProof := '@MANIFEST_DIGEST@|' + GetSHA256OfUnicodeString(BridgeJournal) + #10;
  if EntryIsMissing(BridgeIntent) then begin
    if not EntryIsMissing(Root) or RegKeyExists(InstallRegistryRoot, UninstallKey) then
      Exit;
  end else if not RecordMatches(BridgeIntent, BridgeProof) then begin
    Log('The migration activation attempt does not match the existing intent.');
    Exit;
  end;
  if not BridgeRegistrationMatches(Root) then begin
    Log('The original uninstall registration cannot be reused.');
    Exit;
  end;
  if not EntryIsMissing(Root + '\' + ActiveRuntime) then begin
    Log('The installation already has an activation record.');
    Exit;
  end;
  if not EntryIsMissing(Root) and not BridgeRootIsPlain(Root) then begin
    Log('The prepared installation root is not a plain directory.');
    Exit;
  end;
  Result := EnsureWebView2(NeedsRestart);
  if Result <> '' then
    Exit;
  Result := 'The prepared migration installation could not be resumed safely.';
  if not PublishRecord(BridgeIntent, BridgeProof) then
    Exit;
  if not ReservePreparedRoot(Root) then
    Exit;

  // A complete runtime crosses into the installation root in a single rename.
  // The staged proof and signed worker survive any subsequent Inno rollback.
  Runtime := Root + '\runtime\@APP_VERSION@';
  if not PlainDirectoryChain(ExtractFileDir(Runtime), True) or
    not ForceDirectories(ExtractFileDir(Runtime)) then
    Exit;
  if EntryIsMissing(Runtime) and not PublishStagedRuntime(BridgeStage, Runtime) then
    Exit;
  RepairStage := BridgeStage;
  UpdateStage := BridgeStage;
  UpdateJournal := ExtractFileDir(BridgeStage);
  Bin := Root + '\bin';
  if not RepairFilesMatch(Root, False) then begin
    Log('The previously published migration files do not match the signed stage.');
    Exit;
  end;
  if not CopyRuntimeFile(Bin, '@CLI_FILE@') or not CopyRuntimeFile(Bin, '@CRT_FILE@') or
    not RepairFilesMatch(Root, True) then begin
    Log('The stable command entry could not be completed or reverified.');
    Exit;
  end;

  if not EntryIsMissing(BridgeIntent + '.installed') then begin
    if not RecordMatches(BridgeIntent + '.installed', BridgeProof) or not OwnedUninstallRoot(Root) then
      Exit;
    // This pass only completes the shell entries and selector. Inno must not
    // recreate the uninstall key and discard the entry ownership receipts.
    Repairing := True;
  end;
  Result := '';
end;

procedure CleanFreshBridgeStage;
var
  Remaining: Integer;
begin
  Remaining := @RUNTIME_FILE_COUNT@;
  if RuntimeMatchesStage(BridgeStage, BridgeExtracted, 0, Remaining) and (Remaining = 0) then
    if not DelTree(BridgeStage, True, True, True) then
      Log('Could not remove the verified ALCOMD bridge staging directory.');
end;

function SyncUninstallRegistration: Boolean;
var
  Root, Access, Handle: Cardinal;
begin
  Result := False;
  Root := HKEY_CURRENT_USER;
  Access := KeyQueryValue;
  if IsAdminInstallMode then begin
    // The native API has explicit registry view flags, unlike the Inno registry helpers.
    Root := HKEY_LOCAL_MACHINE;
    Access := Access or Registry64BitView;
  end;
  if OpenRegistryKey(Root, UninstallKey, 0, Access, Handle) <> 0 then
    Exit;
  try
    Result := FlushRegistryKey(Handle) = 0;
  finally
    CloseRegistryKey(Handle);
  end;
end;

procedure PublishStartMenuEntry;
var
  Source, Destination, Digest, Claim, Receipt: String;
begin
  if WizardNoIcons then
    Exit;
  Source := ExpandConstant('{app}\start-menu.lnk');
  Receipt := ExpandConstant('{app}\') + StartMenuReceipt;
  Destination := StartMenuEntry;
  if not PlainDirectoryChain(ExtractFileDir(Destination), False) or
    (GetAttributes(Destination) <> InvalidFileAttributes) or
    not MissingEntry(DLLGetLastError) then
    Exit;
  if not ReadDigest(Source, Digest) then begin
    Log('Could not read the staged Start menu shortcut.');
    Exit;
  end;
  Claim := Digest + '|' + Destination;
  if not (CompletingBridge or EntryIsMissing(Receipt)) or not WriteShellReceipt(Receipt, Claim, '') then
    Exit;
  // RenameFile leaves an existing destination intact.
  if not RenameFile(Source, Destination) then begin
    Log('Could not publish the Start menu shortcut.');
    if not ClearShellReceipt(Receipt, Claim) then
      Log('Could not clear the Start menu ownership receipt.');
  end;
end;

function RemoveOwnedStartMenuEntry: Boolean;
var
  Destination, Receipt, Digest: String;
begin
  Result := True;
  Destination := StartMenuEntry;
  Receipt := ExpandConstant('{app}\') + StartMenuReceipt;
  if not ReadStartMenuReceipt(Receipt, Destination, Digest) then
    Exit;
  if PlainDirectoryChain(ExtractFileDir(Destination), False) and PlainFileMatches(Destination, Digest) then
    if not DeleteFile(Destination) then begin
      Result := False;
      Log('Could not remove the ALCOMD-owned Start menu shortcut.');
      Exit;
    end;
  Result := ClearShellReceipt(Receipt, Digest + '|' + Destination);
  if not Result then
    Log('Could not clear the Start menu ownership receipt.');
end;

@SCOPE_CODE@

procedure ReportIncompleteInstallation(MessageText: String);
begin
  InstallationIncomplete := True;
  Log(MessageText);
  SuppressibleMsgBox(MessageText, mbError, MB_OK, IDOK);
end;

procedure FinishFreshBridge;
var
  Root: String;
begin
  Root := ExpandConstant('{app}');
  if not RecordMatches(BridgeIntent + '.installed', BridgeProof) or
    not OwnedUninstallRoot(Root) or not RepairFilesMatch(Root, True) or not InstallCliPath then begin
    ReportIncompleteInstallation('ALCOMD could not finish publishing the migrated installation.');
    Exit;
  end;
  PublishStartMenuEntry;
  if not PublishProductProtocol('') or not SyncUninstallRegistration or
    (IsAdminInstallMode and not PublishRecord(Root + '\' + MachineBridgeAccount, '1' + #10)) or
    not PublishRecord(Root + '\' + ActiveRuntime, '@APP_VERSION@' + #10) then begin
    ReportIncompleteInstallation('ALCOMD could not activate the migrated installation. Retry with the same bridge.');
    Exit;
  end;
  InstallationIncomplete := False;
  if RecordMatches(BridgeIntent + '.installed', BridgeProof) then
    DeleteFile(BridgeIntent + '.installed');
  if RecordMatches(BridgeIntent, BridgeProof) then
    DeleteFile(BridgeIntent);
  CleanFreshBridgeStage;
end;

function GetCustomSetupExitCode: Integer;
begin
  // Inno treats exceptions raised after copying files as nonfatal.
  Result := 0;
  if InstallationIncomplete then
    Result := IncompleteInstallation;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CommittingUpdate then begin
    if CurStep = ssPostInstall then begin
      InstallationIncomplete := True;
      UpdateEntriesReady := OwnedUninstallRoot(ExpandConstant('{app}')) and
        UpdateStartMenuEntry and PublishProductProtocol(PreviousVersion) and SyncUninstallRegistration;
      if not UpdateEntriesReady then
        ReportIncompleteInstallation('ALCOMD could not finish publishing its update entries.');
    end else if CurStep = ssDone then begin
      if UpdateEntriesReady then begin
        if PublishRecord(UpdateJournal + '\' + UpdateInstalled, '@MANIFEST_DIGEST@' + #10) then
          InstallationIncomplete := False
        else
          ReportIncompleteInstallation('ALCOMD could not persist the update completion receipt.');
      end;
    end;
    Exit;
  end;
  if ConvertingScope then begin
    if CurStep = ssDone then begin
      InstallationIncomplete := True;
      if FinishScopeConversion then
        InstallationIncomplete := False
      else
        ReportIncompleteInstallation('Could not finish Windows scope conversion. Retry with the same installer.');
    end;
    Exit;
  end;
  if CompletingBridge then begin
    if CurStep = ssPostInstall then begin
      InstallationIncomplete := True;
      if not OwnedUninstallRoot(ExpandConstant('{app}')) or not RepairFilesMatch(ExpandConstant('{app}'), True) or
        not SyncUninstallRegistration or not PublishRecord(BridgeIntent + '.installed', BridgeProof) then
        ReportIncompleteInstallation('ALCOMD could not persist the migrated program installation.');
    end else if CurStep = ssDone then
      FinishFreshBridge;
    Exit;
  end;
  if CurStep <> ssPostInstall then
    Exit;
  if Repairing then begin
    if not RepairFilesMatch(ExpandConstant('{app}'), True) or
      (RebuildUninstallRegistration and (not OwnedUninstallRoot(ExpandConstant('{app}')) or
        not SyncUninstallRegistration)) then
      ReportIncompleteInstallation('ALCOMD could not verify the repaired program files.')
    else if not PublishProductProtocol('') then
      ReportIncompleteInstallation('ALCOMD could not repair its link protocol.');
    Exit;
  end;
  if not OwnedUninstallRoot(ExpandConstant('{app}')) or
    not RecordMatches(ExpandConstant('{app}\') + ActiveRuntime, '@APP_VERSION@' + #10) then begin
    ReportIncompleteInstallation('ALCOMD could not establish the active installation version.');
    Exit;
  end;
  if not InstallCliPath then begin
    Log('ALCOMD CLI PATH integration failed. The existing PATH was preserved.');
    SuppressibleMsgBox('ALCOMD was installed, but its CLI could not be added to PATH.',
      mbInformation, MB_OK, IDOK);
  end;
  PublishStartMenuEntry;
  if not PublishProductProtocol('') then
    ReportIncompleteInstallation('ALCOMD could not register its link protocol. Retry with the same installer.');
end;

function AcquireProgramLease(Manifest: String): Boolean;
var
  Range: TLeaseRange;
begin
  Result := False;
  // GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_DELETE, OPEN_EXISTING,
  // FILE_FLAG_OPEN_REPARSE_POINT. Deletion is allowed once Core is excluded.
  ProgramLease := OpenLeaseFile(Manifest, $80000000, 5, 0, 3, $00200000, 0);
  if ProgramLease = -1 then
    Exit;
  Range.Internal := 0;
  Range.InternalHigh := 0;
  Range.Offset := 0;
  Range.OffsetHigh := 1;
  Range.EventHandle := 0;
  // Core shares this byte at 4 GiB. Request an exclusive lock without waiting.
  Result := TryLockLease(ProgramLease, 3, 0, 1, 0, Range);
  if not Result then begin
    CloseKernelHandle(ProgramLease);
    ProgramLease := 0;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Root, Runtime, Manifest, BootstrapCommand, RetirementDigest, ProfileRoot: String;
  ExitCode: Integer;
  Retired, Verified: Boolean;
begin
  Root := ExpandConstant('{app}');
  if CurUninstallStep = usPostUninstall then begin
    if EntryIsMissing(Root + '\' + ActiveRuntime) then begin
      if (UninstallScopePlan <> '') and
        RecordMatches(Root + '\' + ScopeDestinationPrepared, UninstallScopePlan) and
        RecordMatches(Root + '\' + ScopeDestinationCommit, UninstallScopePlan) then
        if DeleteFile(Root + '\' + ScopeDestinationPrepared) then
          DeleteFile(Root + '\' + ScopeDestinationCommit);
      if (RetiredUninstallDigest <> '') and
        PlainFileMatches(Root + '\' + ScopeTransition, RetiredUninstallDigest) then
        DeleteFile(Root + '\' + ScopeTransition);
      if IsAdminInstallMode and RecordMatches(Root + '\' + MachineBridgeAccount, '1' + #10) then
        DeleteFile(Root + '\' + MachineBridgeAccount);
      if RecordMatches(UninstallRootGuardPath, '@MANIFEST_DIGEST@' + #10) and
        RunRootWorker('release', Root) then begin
        if not IsAdminInstallMode and
          RegQueryStringValue(HKEY_CURRENT_USER, UserScopeLocatorKey, 'OwnedRoot', ProfileRoot) and
          (CompareText(ProfileRoot, Root) = 0) then
          RegDeleteValue(HKEY_CURRENT_USER, UserScopeLocatorKey, 'OwnedRoot');
        if DeleteFile(UninstallRootGuardPath) then
          RemoveDir(Root);
      end else
        Log('The ALCOMD installation locator was kept for recovery.');
    end;
    Exit;
  end;
  if CurUninstallStep <> usUninstall then
    Exit;
  Runtime := Root + '\runtime\@APP_VERSION@';
  Manifest := Runtime + '\@MANIFEST_NAME@';
  RetirementDigest := ExpandConstant('{param:ALCOMDRetireScope|}');
  Retired := RetirementDigest <> '';
  if Retired then
    Verified := LowerDigest(RetirementDigest) and
      PlainFileMatches(Root + '\' + ScopeTransition, RetirementDigest) and
      EntryIsMissing(Root + '\' + ActiveRuntime)
  else
    Verified := EntryIsMissing(Root + '\' + ScopeTransition) and
      RecordMatches(Root + '\' + ActiveRuntime, '@APP_VERSION@' + #10);
  if not MatchesReservedRoot(Root) or
    (RegKeyExists(InstallRegistryRoot, UninstallKey) and not OwnedUninstallRoot(Root)) or
    not PlainDirectoryChain(Runtime, False) or
    not PlainDirectoryChain(Root + '\bin', False) or
    not EntryIsMissing(Root + '\' + UpdateIntent) or
    not Verified or
    not PlainFileMatches(Manifest, '@MANIFEST_DIGEST@') then begin
    Log('Could not confirm installed ALCOMD program ownership. No files were removed.');
    SuppressibleMsgBox('The installed ALCOMD files could not be verified.',
      mbError, MB_OK, IDOK);
    Abort;
  end;
  if not AcquireProgramLease(Manifest) then begin
    Log('Could not lock the installed ALCOMD runtime. No files were removed.');
    SuppressibleMsgBox('Close ALCOMD in all signed-in accounts and try uninstalling again.',
      mbError, MB_OK, IDOK);
    Abort;
  end;
  if not PreserveUninstallRootWorker(Runtime) or not RunRootWorker('check', Root) then begin
    Log('The protected ALCOMD installation locator could not be verified. No files were removed.');
    Abort;
  end;
  if not EntryIsMissing(Root + '\' + ScopeDestinationPrepared) or
    not EntryIsMissing(Root + '\' + ScopeDestinationCommit) then
    if not RecordText(Root + '\' + ScopeDestinationPrepared, 8192, UninstallScopePlan) or
      not RecordMatches(Root + '\' + ScopeDestinationCommit, UninstallScopePlan) then begin
      Log('Could not verify the committed Windows scope records. No files were removed.');
      Abort;
    end;
  if Retired then begin
    BootstrapCommand := 'verify-retired-windows-scope-uninstall --approved-plan ' + RetirementDigest;
    if IsAdminInstallMode then
      BootstrapCommand := BootstrapCommand + ' --machine';
  end else if IsAdminInstallMode then
    BootstrapCommand := 'verify-installed-uninstall'
  else
    BootstrapCommand := 'unregister-installed-core';
  Verified := False;
  if PlainFileMatches(Runtime + '\@BOOTSTRAP_FILE@', '@BOOTSTRAP_DIGEST@') and
    PlainFileMatches(Runtime + '\@CRT_FILE@', '@CRT_DIGEST@') then
    if Exec(Runtime + '\@BOOTSTRAP_FILE@',
      BootstrapCommand + ' --source-sha @SOURCE_SHA@', Runtime, SW_HIDE,
      ewWaitUntilTerminated, ExitCode) then
      Verified := ExitCode = 0;
  if not Verified then begin
    Log('Could not verify all ALCOMD program files or update the owned registration. The installation was kept.');
    SuppressibleMsgBox('The installed ALCOMD files or registration could not be verified.',
      mbError, MB_OK, IDOK);
    Abort;
  end;
  // Keep the protected directory identity available until post-uninstall releases its locator.
  UninstallRootGuardPath := Root + '\' + RootUninstallGuard;
  if not RecordMatches(UninstallRootGuardPath, '@MANIFEST_DIGEST@' + #10) and
    (not EntryIsMissing(UninstallRootGuardPath) or
      not PublishRecord(UninstallRootGuardPath, '@MANIFEST_DIGEST@' + #10)) then
    Abort;
  if not RemoveOwnedCliPath then begin
    Log('Could not remove the ALCOMD-owned CLI PATH entry.');
    SuppressibleMsgBox('The ALCOMD CLI entry could not be removed from PATH.',
      mbInformation, MB_OK, IDOK);
    if Retired then
      Abort;
  end;
  if not RemoveOwnedStartMenuEntry and Retired then
    Abort;
  if not RemoveOwnedProductProtocol(Root) then begin
    Log('Could not remove the ALCOMD-owned link protocol.');
    Abort;
  end;
  if Retired then begin
    RetiredUninstallDigest := RetirementDigest;
    Exit;
  end;
  if not RecordMatches(Root + '\' + ActiveRuntime, '@APP_VERSION@' + #10) or
    not DeleteFile(Root + '\' + ActiveRuntime) then begin
    Log('Could not remove the ALCOMD active-runtime record.');
    Abort;
  end;
end;

procedure ReleaseProgramLease;
begin
  if (ProgramLease <> 0) and (ProgramLease <> -1) then
    CloseKernelHandle(ProgramLease);
end;

procedure DeinitializeSetup;
begin
  RestoreUncommittedScope;
  ReleaseProgramLease;
end;

procedure DeinitializeUninstall;
begin
  ReleaseProgramLease;
  if (UninstallRootGuardPath <> '') and RecordMatches(UninstallRootGuardPath, '@MANIFEST_DIGEST@' + #10) then
    DeleteFile(UninstallRootGuardPath);
end;
