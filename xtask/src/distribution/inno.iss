[Setup]
AppId={@APP_ID@
AppName=@APP_NAME@
AppVersion=@APP_VERSION@
AppPublisher=@APP_PUBLISHER@
DefaultDirName={autopf}\@INSTALL_DIRECTORY@
DisableDirPage=yes
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
OutputBaseFilename=@FAMILY_NAME@_@APP_VERSION@_windows_x86_64_setup

[Files]
@FILES@
Source: "active-runtime-v1"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall; Check: IsFreshInstallation

[Icons]
Name: "{app}\start-menu"; Filename: "{app}\runtime\@APP_VERSION@\@GUI_NAME@.exe"; AppUserModelID: "@APP_AUMID@"; Flags: createonlyiffileexists; Check: CreateFreshIcon
Name: "{tmp}\candidate-start-menu"; Filename: "{app}\runtime\@APP_VERSION@\@GUI_NAME@.exe"; AppUserModelID: "@APP_AUMID@"; Flags: createonlyiffileexists uninsneveruninstall; Check: CreateUpdateIcon

[UninstallDelete]
@UNINSTALL_FILES@

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\@APP_ID@_is1';
  ActiveRuntime = 'active-runtime-v1';
  UpdateIntent = 'update-intent-v1';
  ShellReceipts = 'previous-shell-v1';
  UpdateInstalled = 'installed-v1';
  PreviousShortcutFile = 'previous-start-menu.lnk';
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
  OwnedCliPath = '@FAMILY_NAME@OwnedCliPath';
  OwnedStartMenu = '@FAMILY_NAME@OwnedStartMenu';
  StringValue = 1;
  ExpandStringValue = 2;
  ReadEnvironmentString = $10000006;
  KeyQueryValue = 1;
  Registry64BitView = $0100;

type
  TLeaseRange = record
    Internal, InternalHigh, Offset, OffsetHigh, EventHandle: Cardinal;
  end;

var
  UninstallLease: Integer;
  CommittingUpdate, UpdateEntriesReady: Boolean;
  PreviousVersion, PreviousDigest, UpdateJournal, UpdateStage: String;
  PreviousCli, PreviousShortcut: String;

function OpenLeaseFile(FileName: String; Access, Share: Cardinal; Security: Integer;
  Disposition, Flags: Cardinal; Template: Integer): Integer;
  external 'CreateFileW@kernel32.dll stdcall';

function TryLockLease(Handle: Integer; Flags, Reserved, LengthLow, LengthHigh: Cardinal;
  var Range: TLeaseRange): Boolean;
  external 'LockFileEx@kernel32.dll stdcall';

function CloseLeaseFile(Handle: Integer): Boolean;
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

function RecordText(Path: String; MaxSize: Integer; var Content: String): Boolean;
var
  Size: Int64;
  Bytes: AnsiString;
begin
  Result := PlainFile(Path) and FileSize64(Path, Size) and (Size > 0) and
    (Size <= MaxSize) and LoadStringFromFile(Path, Bytes) and (Length(Bytes) = Size);
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
    CloseLeaseFile(Handle);
  end;
end;

function PublishRecord(Path, Content: String): Boolean;
var
  Temporary: String;
begin
  Result := RecordMatches(Path, Content);
  if Result or not EntryIsMissing(Path) then
    Exit;
  Temporary := GenerateUniqueName(AddBackslash(ExtractFileDir(Path)), '.tmp');
  if Temporary = '' then
    Exit;
  if not SaveStringToFile(Temporary, AnsiString(Content), False) then
    Exit;
  try
    Result := SyncPlainFile(Temporary) and PublishFile(Temporary, Path, WriteThrough);
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

function ConflictWithLoadedProfile: Boolean;
var
  Profiles: TArrayOfString;
  Index: Integer;
begin
  Result := True;
  if not RegGetSubkeyNames(HKEY_USERS, '', Profiles) then
    Exit;
  for Index := 0 to GetArrayLength(Profiles) - 1 do
    if RegKeyExists(HKEY_USERS, Profiles[Index] + '\' + UninstallKey) then
      Exit;
  Result := False;
end;

function MachineInstallationPresent: Boolean;
begin
  // An interrupted update can have deleted the machine uninstall key without
  // removing its installation. A different user must still see the conflict.
  Result := RegKeyExists(HKEY_LOCAL_MACHINE_64, UninstallKey) or
    not EntryIsMissing(ExpandConstant('{commonpf}\@INSTALL_DIRECTORY@'));
end;

function PrepareUpdate(Root: String): Boolean;
  forward;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Root: String;
begin
  Result := 'Use the standard ALCOMD installation folder on a local fixed drive.';
  Root := RemoveBackslashUnlessRoot(WizardDirValue);
  if (CompareText(Root, ExpandConstant('{autopf}\@INSTALL_DIRECTORY@')) <> 0) or
    not PlainDirectoryChain(ExtractFileDir(Root), True) then
    Exit;

  if CommittingUpdate then begin
    Result := 'The ALCOMD update could not verify the existing installation or its recovery records.';
    if PrepareUpdate(Root) then
      Result := '';
    Exit;
  end;

  Result := 'The destination already exists or could not be checked. Its contents were kept.';
  if GetAttributes(Root) <> InvalidFileAttributes then
    Exit;
  if not MissingEntry(DLLGetLastError) then
    Exit;

  Result := 'An existing ALCOMD installation was found or could not be ruled out.';
  if MachineInstallationPresent or RegKeyExists(HKEY_CURRENT_USER, UninstallKey) then
    Exit;
  if IsAdminInstallMode and ConflictWithLoadedProfile then
    Exit;
  Result := '';
end;

function OwnedUninstallRoot(Root: String): Boolean;
var
  RegisteredRoot: String;
begin
  Result := False;
  if IsAdminInstallMode then begin
    if CompareText(Root, ExpandConstant('{commonpf}\@INSTALL_DIRECTORY@')) <> 0 then
      Exit;
    if not RegQueryStringValue(HKEY_LOCAL_MACHINE_64, UninstallKey, 'InstallLocation', RegisteredRoot) then
      Exit;
  end else begin
    if CompareText(Root, ExpandConstant('{localappdata}\Programs\@INSTALL_DIRECTORY@')) <> 0 then
      Exit;
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
  Digest, Commit, Scope, Root, Destination, Extracted: String;
begin
  Digest := ExpandConstant('{param:ALCOMDStage|}');
  Commit := ExpandConstant('{param:ALCOMDCommit|}');
  Scope := ExpandConstant('{param:ALCOMDScope|}');
  PreviousVersion := ExpandConstant('{param:ALCOMDPrevious|}');
  PreviousDigest := ExpandConstant('{param:ALCOMDPreviousDigest|}');
  CommittingUpdate := Commit <> '';
  Result := (Digest = '') and (Commit = '') and (Scope = '') and
    (PreviousVersion = '') and (PreviousDigest = '');
  if Result then
    Exit;
  // Bind private update modes to the requested scope before accessing either installation.
  if ((Scope = '') and IsAdminInstallMode) or
    ((Scope <> '') and ((Scope <> 'all-users') or not IsAdminInstallMode)) then
    Exit;
  if CommittingUpdate then begin
    Result := (Digest = '') and
      (Commit = '@MANIFEST_DIGEST@') and PlainVersion(PreviousVersion) and
      (PreviousVersion <> '@APP_VERSION@') and LowerDigest(PreviousDigest);
    Exit;
  end;
  // Bootstrap verifies this installer and supplies the expected payload digest.
  // Both success and failure leave the normal Inno installation path unused.
  if (Digest <> '@MANIFEST_DIGEST@') or (PreviousVersion <> '') or (PreviousDigest <> '') then
    Exit;
  Root := ExpandConstant('{autopf}\@INSTALL_DIRECTORY@');
  if not OwnedUninstallRoot(Root) or not PlainDirectoryChain(Root, False) or UpdateScopeConflicts then
    Exit;
  Destination := AddBackslash(ExtractFileDir(Root)) +
    '.@INSTALL_DIRECTORY@@UPDATE_STAGE_PREFIX@' + Digest;
  if not EntryIsMissing(Destination) then
    Exit;
  ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*');
  Extracted := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  if not PublishStagedRuntime(Extracted, Destination) then
    Log('Could not publish the ALCOMD update staging directory.');
end;

function IsFreshInstallation: Boolean;
begin
  Result := not CommittingUpdate;
end;

function IsUpdateInstallation: Boolean;
begin
  Result := CommittingUpdate;
end;

function CreateFreshIcon: Boolean;
begin
  Result := not CommittingUpdate and not WizardNoIcons;
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

function InstallCliPath: Boolean;
var
  Root: Cardinal;
  Entry, Path, Claim: String;
  ValueType: Cardinal;
begin
  Result := False;
  Root := InstallRegistryRoot;
  Entry := ExpandConstant('{app}\bin');
  if (Pos(';', Entry) <> 0) or (Pos('%', Entry) <> 0) or not ReadPath(Path, ValueType) then
    Exit;
  if PathEntryIsPresent(Path, Entry) then begin
    Result := True;
    Exit;
  end;
  if RegValueExists(Root, UninstallKey, OwnedCliPath) then
    Exit;
  Claim := 'existing|' + Entry;
  if ValueType = 0 then begin
    Claim := 'missing|' + Entry;
    ValueType := ExpandStringValue;
  end;
  if Path <> '' then
    Path := Path + ';';
  Path := Path + Entry;
  if (Length(Path) > 32766) or
    not RegWriteStringValue(Root, UninstallKey, OwnedCliPath, Claim) then
    Exit;
  Result := WritePath(Path, ValueType);
  if not Result then
    if not RegDeleteValue(Root, UninstallKey, OwnedCliPath) then
      Log('Could not clear the CLI PATH ownership receipt.');
end;

function RemoveOwnedCliPath: Boolean;
var
  Root: Cardinal;
  Entry, Path, Claim, Token: String;
  ValueType: Cardinal;
  Position: Integer;
begin
  Result := True;
  Root := InstallRegistryRoot;
  if not RegQueryStringValue(Root, UninstallKey, OwnedCliPath, Claim) then
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
  if Position = 0 then
    Exit;
  Delete(Path, Position, Length(Token) - 1);
  if Length(Path) > 1 then
    Path := Copy(Path, 2, Length(Path) - 2)
  else
    Path := '';
  if (Path = '') and (Claim = 'missing|' + Entry) then
    ValueType := 0;
  Result := WritePath(Path, ValueType);
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

function SnapshotShellEntries(Root: String): Boolean;
var
  Claim, Entry, Path, Shortcut: String;
  ValueType, RegistryRoot: Cardinal;
begin
  Result := False;
  RegistryRoot := InstallRegistryRoot;
  PreviousCli := 'none';
  PreviousShortcut := 'none';
  Entry := Root + '\bin';
  if not ReadPath(Path, ValueType) then
    Exit;
  if RegQueryStringValue(RegistryRoot, UninstallKey, OwnedCliPath, Claim) and
    ((Claim = 'existing|' + Entry) or (Claim = 'missing|' + Entry)) and
    (ExactPathSlot(';' + Path + ';', ';' + Entry + ';') <> 0) then
    PreviousCli := Copy(Claim, 1, Pos('|', Claim) - 1);
  Shortcut := StartMenuEntry;
  if RegQueryStringValue(RegistryRoot, UninstallKey, OwnedStartMenu, Claim) and
    (Copy(Claim, 65, Length(Claim)) = '|' + Shortcut) and
    PlainDirectoryChain(ExtractFileDir(Shortcut), False) then begin
    Claim := Copy(Claim, 1, 64);
    if LowerDigest(Claim) and PlainFileMatches(Shortcut, Claim) then begin
      if not PublishCopy(Shortcut, UpdateJournal + '\' + PreviousShortcutFile, Claim, '', UpdateJournal) then
        Exit;
      PreviousShortcut := Claim;
    end;
  end;
  Result := PublishRecord(UpdateJournal + '\' + ShellReceipts,
    PreviousCli + #10 + PreviousShortcut + #10);
end;

function ReadShellSnapshot: Boolean;
var
  Content: String;
  Delimiter: Integer;
begin
  Result := False;
  if not RecordText(UpdateJournal + '\' + ShellReceipts, 74, Content) then
    Exit;
  Delimiter := Pos(#10, Content);
  if (Delimiter = 0) or (Content[Length(Content)] <> #10) then
    Exit;
  PreviousCli := Copy(Content, 1, Delimiter - 1);
  PreviousShortcut := Copy(Content, Delimiter + 1, Length(Content) - Delimiter - 1);
  Result := ((PreviousCli = 'existing') or (PreviousCli = 'missing') or (PreviousCli = 'none')) and
    ((PreviousShortcut = 'none') or (LowerDigest(PreviousShortcut) and
      PlainFileMatches(UpdateJournal + '\' + PreviousShortcutFile, PreviousShortcut)));
end;

function RestoreCliReceipt(Root: String): Boolean;
var
  Entry, Path: String;
  ValueType: Cardinal;
begin
  Result := True;
  if PreviousCli = 'none' then
    Exit;
  Result := ReadPath(Path, ValueType);
  if not Result then
    Exit;
  Entry := Root + '\bin';
  if ExactPathSlot(';' + Path + ';', ';' + Entry + ';') <> 0 then
    Result := RegWriteStringValue(InstallRegistryRoot, UninstallKey, OwnedCliPath, PreviousCli + '|' + Entry);
end;

function UpdateStartMenuEntry: Boolean;
var
  Candidate, Destination, Digest, Extracted: String;
begin
  Result := True;
  if PreviousShortcut = 'none' then
    Exit;
  Destination := StartMenuEntry;
  if not PlainDirectoryChain(ExtractFileDir(Destination), False) then
    Exit;
  Candidate := UpdateJournal + '\' + CandidateShortcutFile;
  if EntryIsMissing(Candidate) then begin
    // Inno produces this link with the permanent AUMID. Keep its exact bytes for retries.
    Extracted := ExpandConstant('{tmp}\') + CandidateShortcutFile;
    Result := ReadDigest(Extracted, Digest) and PublishCopy(Extracted, Candidate, Digest, '', UpdateJournal);
  end else
    Result := ReadDigest(Candidate, Digest);
  if not Result then
    Exit;
  if PlainFileMatches(Destination, PreviousShortcut) then begin
    Result := PublishCopy(Candidate, Destination, Digest, PreviousShortcut, ExtractFileDir(Destination));
    if not Result then
      Exit;
  end;
  // If the user replaced or removed the prior link, preserve their choice.
  if PlainFileMatches(Destination, Digest) then
    Result := RegWriteStringValue(InstallRegistryRoot, UninstallKey, OwnedStartMenu, Digest + '|' + Destination);
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
    CloseLeaseFile(Handle);
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

function PrepareUpdate(Root: String): Boolean;
var
  OldRuntime, NewRuntime, CandidateRuntime, Manifest, Intent: String;
  Resuming: Boolean;
  Remaining: Integer;
begin
  Result := False;
  if not PlainDirectoryChain(Root + '\bin', False) then
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
  Resuming := not EntryIsMissing(UpdateJournal + '\' + ShellReceipts);
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
  if Resuming then begin
    if not ReadShellSnapshot then
      Exit;
  end else if not SnapshotShellEntries(Root) then
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
  Root: Cardinal;
  Source, Destination, Digest: String;
begin
  if WizardNoIcons then
    Exit;
  Root := InstallRegistryRoot;
  Source := ExpandConstant('{app}\start-menu.lnk');
  Destination := StartMenuEntry;
  if not PlainDirectoryChain(ExtractFileDir(Destination), False) or
    (GetAttributes(Destination) <> InvalidFileAttributes) or
    not MissingEntry(DLLGetLastError) or
    RegValueExists(Root, UninstallKey, OwnedStartMenu) then
    Exit;
  if not ReadDigest(Source, Digest) then begin
    Log('Could not read the staged Start menu shortcut.');
    Exit;
  end;
  if not RegWriteStringValue(Root, UninstallKey, OwnedStartMenu, Digest + '|' + Destination) then
    Exit;
  // RenameFile leaves an existing destination intact.
  if not RenameFile(Source, Destination) then begin
    Log('Could not publish the Start menu shortcut.');
    if not RegDeleteValue(Root, UninstallKey, OwnedStartMenu) then
      Log('Could not clear the Start menu ownership receipt.');
  end;
end;

procedure RemoveOwnedStartMenuEntry;
var
  Destination, Claim: String;
begin
  Destination := StartMenuEntry;
  if RegQueryStringValue(InstallRegistryRoot, UninstallKey, OwnedStartMenu, Claim) and
    (Copy(Claim, 65, Length(Claim)) = '|' + Destination) and
    PlainDirectoryChain(ExtractFileDir(Destination), False) and
    PlainFileMatches(Destination, Copy(Claim, 1, 64)) then
    if not DeleteFile(Destination) then
      Log('Could not remove the ALCOMD-owned Start menu shortcut.');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CommittingUpdate then begin
    if CurStep = ssPostInstall then begin
      UpdateEntriesReady := OwnedUninstallRoot(ExpandConstant('{app}')) and
        RestoreCliReceipt(ExpandConstant('{app}')) and UpdateStartMenuEntry and
        SyncUninstallRegistration;
      if not UpdateEntriesReady then
        RaiseException('ALCOMD could not finish publishing its update entries.');
    end else if CurStep = ssDone then begin
      if not UpdateEntriesReady or
        not PublishRecord(UpdateJournal + '\' + UpdateInstalled, '@MANIFEST_DIGEST@' + #10) then
        RaiseException('ALCOMD could not persist the update completion receipt.');
    end;
    Exit;
  end;
  if CurStep <> ssPostInstall then
    Exit;
  if not OwnedUninstallRoot(ExpandConstant('{app}')) then
    Exit;
  if not RecordMatches(ExpandConstant('{app}\') + ActiveRuntime, '@APP_VERSION@' + #10) then
    RaiseException('ALCOMD could not establish the active installation version.');
  if not InstallCliPath then begin
    Log('ALCOMD CLI PATH integration failed. The existing PATH was preserved.');
    SuppressibleMsgBox('ALCOMD was installed, but its CLI could not be added to PATH.',
      mbInformation, MB_OK, IDOK);
  end;
  PublishStartMenuEntry;
end;

function AcquireUninstallLease(Manifest: String): Boolean;
var
  Range: TLeaseRange;
begin
  Result := False;
  // GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_DELETE, OPEN_EXISTING,
  // FILE_FLAG_OPEN_REPARSE_POINT. Deletion is allowed once Core is excluded.
  UninstallLease := OpenLeaseFile(Manifest, $80000000, 5, 0, 3, $00200000, 0);
  if UninstallLease = -1 then
    Exit;
  Range.Internal := 0;
  Range.InternalHigh := 0;
  Range.Offset := 0;
  Range.OffsetHigh := 1;
  Range.EventHandle := 0;
  // Core shares this byte at 4 GiB. Request an exclusive lock without waiting.
  Result := TryLockLease(UninstallLease, 3, 0, 1, 0, Range);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Root, Runtime, Manifest, BootstrapCommand: String;
  ExitCode: Integer;
  Verified: Boolean;
begin
  if CurUninstallStep <> usUninstall then
    Exit;
  Root := ExpandConstant('{app}');
  Runtime := Root + '\runtime\@APP_VERSION@';
  Manifest := Runtime + '\@MANIFEST_NAME@';
  if not OwnedUninstallRoot(Root) or not PlainDirectoryChain(Runtime, False) or
    not PlainDirectoryChain(Root + '\bin', False) or
    not EntryIsMissing(Root + '\' + UpdateIntent) or
    not RecordMatches(Root + '\' + ActiveRuntime, '@APP_VERSION@' + #10) or
    not PlainFileMatches(Manifest, '@MANIFEST_DIGEST@') then begin
    Log('Could not confirm installed ALCOMD program ownership. No files were removed.');
    SuppressibleMsgBox('The installed ALCOMD files could not be verified.',
      mbError, MB_OK, IDOK);
    Abort;
  end;
  if not AcquireUninstallLease(Manifest) then begin
    Log('Could not lock the installed ALCOMD runtime. No files were removed.');
    SuppressibleMsgBox('Close ALCOMD in all signed-in accounts and try uninstalling again.',
      mbError, MB_OK, IDOK);
    Abort;
  end;
  if IsAdminInstallMode then
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
  if not RemoveOwnedCliPath then begin
    Log('Could not remove the ALCOMD-owned CLI PATH entry.');
    SuppressibleMsgBox('The ALCOMD CLI entry could not be removed from PATH.',
      mbInformation, MB_OK, IDOK);
  end;
  RemoveOwnedStartMenuEntry;
  if not RecordMatches(Root + '\' + ActiveRuntime, '@APP_VERSION@' + #10) or
    not DeleteFile(Root + '\' + ActiveRuntime) then begin
    Log('Could not remove the ALCOMD active-runtime record.');
    Abort;
  end;
end;

procedure DeinitializeUninstall;
begin
  if (UninstallLease <> 0) and (UninstallLease <> -1) then
    CloseLeaseFile(UninstallLease);
end;
