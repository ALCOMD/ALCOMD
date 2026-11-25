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

[Icons]
Name: "{app}\start-menu"; Filename: "{app}\runtime\@APP_VERSION@\@GUI_NAME@.exe"; AppUserModelID: "@APP_AUMID@"; Flags: createonlyiffileexists; Check: not WizardNoIcons

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\@APP_ID@_is1';
  MissingFile = 2;
  MissingPath = 3;
  FixedDrive = 3;
  InvalidFileAttributes = $FFFFFFFF;
  UserEnvironment = 'Environment';
  MachineEnvironment = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';
  OwnedCliPath = '@FAMILY_NAME@OwnedCliPath';
  OwnedStartMenu = '@FAMILY_NAME@OwnedStartMenu';
  StringValue = 1;
  ExpandStringValue = 2;
  ReadEnvironmentString = $10000006;

type
  TLeaseRange = record
    Internal, InternalHigh, Offset, OffsetHigh, EventHandle: Cardinal;
  end;

var
  UninstallLease: Integer;

function OpenLeaseFile(FileName: String; Access, Share: Cardinal; Security: Integer;
  Disposition, Flags: Cardinal; Template: Integer): Integer;
  external 'CreateFileW@kernel32.dll stdcall';

function TryLockLease(Handle: Integer; Flags, Reserved, LengthLow, LengthHigh: Cardinal;
  var Range: TLeaseRange): Boolean;
  external 'LockFileEx@kernel32.dll stdcall';

function CloseLeaseFile(Handle: Integer): Boolean;
  external 'CloseHandle@kernel32.dll stdcall';

function GetAttributes(FileName: String): Cardinal;
  external 'GetFileAttributesW@kernel32.dll stdcall';

function GetDriveType(Root: String): Cardinal;
  external 'GetDriveTypeW@kernel32.dll stdcall';

function ReadRegistryValueType(Root: Cardinal; Key, Name: String; Flags: Cardinal;
  var ValueType: Cardinal; Data: Integer; var Size: Cardinal): Integer;
  external 'RegGetValueW@advapi32.dll stdcall';

function MissingEntry(Error: Integer): Boolean;
begin
  Result := (Error = MissingFile) or (Error = MissingPath);
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

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Root: String;
begin
  Result := 'Use the standard ALCOMD installation folder on a local fixed drive.';
  Root := RemoveBackslashUnlessRoot(WizardDirValue);
  if (CompareText(Root, ExpandConstant('{autopf}\@INSTALL_DIRECTORY@')) <> 0) or
    not PlainDirectoryChain(ExtractFileDir(Root), True) then
    Exit;

  Result := 'The destination already exists or could not be checked. Its contents were kept.';
  if GetAttributes(Root) <> InvalidFileAttributes then
    Exit;
  if not MissingEntry(DLLGetLastError) then
    Exit;

  Result := 'An existing ALCOMD installation was found or could not be ruled out.';
  if RegKeyExists(HKEY_LOCAL_MACHINE_64, UninstallKey) or RegKeyExists(HKEY_CURRENT_USER, UninstallKey) then
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

function InitializeSetup: Boolean;
var
  Digest, Root, Destination, Extracted: String;
begin
  Digest := ExpandConstant('{param:ALCOMDStage|}');
  Result := Digest = '';
  if Result then
    Exit;
  // Bootstrap verifies this installer and supplies the expected payload digest.
  // Both success and failure leave the normal Inno installation path unused.
  if IsAdminInstallMode or (Digest <> '@MANIFEST_DIGEST@') then
    Exit;
  Root := ExpandConstant('{localappdata}\Programs\@INSTALL_DIRECTORY@');
  if not OwnedUninstallRoot(Root) or not PlainDirectoryChain(Root, False) then
    Exit;
  Destination := AddBackslash(ExtractFileDir(Root)) +
    '.@INSTALL_DIRECTORY@@UPDATE_STAGE_PREFIX@' + Digest;
  if (GetAttributes(Destination) <> InvalidFileAttributes) or not MissingEntry(DLLGetLastError) then
    Exit;
  ExtractTemporaryFiles('{app}\runtime\@APP_VERSION@\*');
  Extracted := ExpandConstant('{tmp}\') + '{app}\runtime\@APP_VERSION@';
  if not RenameFile(Extracted, Destination) then
    Log('Could not publish the ALCOMD update staging directory.');
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
  Position := Pos(Token, Path);
  if Position = 0 then
    Exit;
  if Pos(Token, Copy(Path, Position + Length(Token) - 1, Length(Path))) <> 0 then
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

function PlainFileMatches(FileName, Digest: String): Boolean;
var
  Attributes: Cardinal;
begin
  Result := False;
  Attributes := GetAttributes(FileName);
  if (Attributes = InvalidFileAttributes) or
    (Attributes and (FILE_ATTRIBUTE_DIRECTORY or FILE_ATTRIBUTE_REPARSE_POINT) <> 0) then
    Exit;
  try
    Result := CompareText(GetSHA256OfFile(FileName), Digest) = 0;
  except
    Log('Could not read an installed ALCOMD file.');
  end;
end;

function StartMenuEntry: String;
begin
  Result := ExpandConstant('{autoprograms}\@APP_NAME@.lnk');
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
  try
    Digest := GetSHA256OfFile(Source);
  except
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
  if CurStep <> ssPostInstall then
    Exit;
  if not OwnedUninstallRoot(ExpandConstant('{app}')) then
    Exit;
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
  Root, Runtime, Manifest: String;
  ExitCode: Integer;
  Unregistered: Boolean;
begin
  if CurUninstallStep <> usUninstall then
    Exit;
  Root := ExpandConstant('{app}');
  Runtime := Root + '\runtime\@APP_VERSION@';
  Manifest := Runtime + '\@MANIFEST_NAME@';
  if not OwnedUninstallRoot(Root) or not PlainDirectoryChain(Runtime, False) or
    not PlainDirectoryChain(Root + '\bin', False) or
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
  if not IsAdminInstallMode then begin
    Unregistered := False;
    if PlainFileMatches(Runtime + '\@BOOTSTRAP_FILE@', '@BOOTSTRAP_DIGEST@') and
      PlainFileMatches(Runtime + '\@CRT_FILE@', '@CRT_DIGEST@') then
      if Exec(Runtime + '\@BOOTSTRAP_FILE@',
        'unregister-installed-core --source-sha @SOURCE_SHA@', Runtime, SW_HIDE,
        ewWaitUntilTerminated, ExitCode) then
        Unregistered := ExitCode = 0;
    if not Unregistered then begin
      Log('Could not unregister the current account. The installation was kept.');
      SuppressibleMsgBox('Close ALCOMD and try uninstalling again.', mbError, MB_OK, IDOK);
      Abort;
    end;
  end;
  if not RemoveOwnedCliPath then begin
    Log('Could not remove the ALCOMD-owned CLI PATH entry.');
    SuppressibleMsgBox('The ALCOMD CLI entry could not be removed from PATH.',
      mbInformation, MB_OK, IDOK);
  end;
  RemoveOwnedStartMenuEntry;
end;

procedure DeinitializeUninstall;
begin
  if (UninstallLease <> 0) and (UninstallLease <> -1) then
    CloseLeaseFile(UninstallLease);
end;
