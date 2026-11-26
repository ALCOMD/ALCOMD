function BeginProtocolTransaction(Security, UnitOfWork, Options, Level, Flags, Timeout: Cardinal;
  Description: String): Integer;
  external 'CreateTransaction@ktmw32.dll stdcall';

function CommitProtocolTransaction(Transaction: Integer): Boolean;
  external 'CommitTransaction@ktmw32.dll stdcall';

function RollbackProtocolTransaction(Transaction: Integer): Boolean;
  external 'RollbackTransaction@ktmw32.dll stdcall';

function CreateProtocolKey(Root: Cardinal; Key: String; Reserved, KeyClass, Options, Access,
  Security: Cardinal; var Handle, Disposition: Cardinal; Transaction: Integer;
  Extended: Cardinal): Integer;
  external 'RegCreateKeyTransactedW@advapi32.dll stdcall';

function OpenProtocolKey(Root: Cardinal; Key: String; Options, Access: Cardinal;
  var Handle: Cardinal; Transaction: Integer; Extended: Cardinal): Integer;
  external 'RegOpenKeyTransactedW@advapi32.dll stdcall';

function DeleteProtocolKey(Root: Cardinal; Key: String; View, Reserved: Cardinal;
  Transaction: Integer; Extended: Cardinal): Integer;
  external 'RegDeleteKeyTransactedW@advapi32.dll stdcall';

function QueryProtocolKeyCounts(Handle, KeyClass, ClassLength, Reserved: Cardinal;
  var Subkeys: Cardinal; MaxSubkeyLength, MaxClassLength: Cardinal; var Values: Cardinal;
  MaxValueNameLength, MaxValueLength, SecurityLength, LastWriteTime: Cardinal): Integer;
  external 'RegQueryInfoKeyW@advapi32.dll stdcall';

function QueryProtocolValue(Handle: Cardinal; Name: String; Reserved: Cardinal;
  var ValueType: Cardinal; Value: String; var Size: Cardinal): Integer;
  external 'RegQueryValueExW@advapi32.dll stdcall';

function SetProtocolOwner(Handle: Cardinal; Name: String; Reserved, ValueType: Cardinal;
  Value: String; Size: Cardinal): Integer;
  external 'RegSetValueExW@advapi32.dll stdcall';

procedure NotifyProtocolChanged(EventId, Flags, Item1, Item2: Cardinal);
  external 'SHChangeNotify@shell32.dll stdcall';

procedure ProtocolRegistryView(RegistryRoot: Cardinal; var Root, View: Cardinal);
begin
  Root := RegistryRoot;
  View := 0;
  if RegistryRoot = HKEY_LOCAL_MACHINE_64 then begin
    Root := HKEY_LOCAL_MACHINE;
    View := Registry64BitView;
  end;
end;

function ProtocolKeyStatus(RegistryRoot: Cardinal; Key: String): Integer;
var
  Root, View, Handle: Cardinal;
begin
  ProtocolRegistryView(RegistryRoot, Root, View);
  Result := OpenRegistryKey(Root, Key, 0, KeyQueryValue or View, Handle);
  if Result = 0 then
    CloseRegistryKey(Handle);
end;

function ProtocolHandleValueMatches(Handle: Cardinal; Name, Expected: String): Boolean;
var
  Kind, Size, ExpectedSize: Cardinal;
  Value: String;
begin
  // Use the raw value so the exact type, size and single UTF-16 terminator are checked together.
  ExpectedSize := 2 * (Length(Expected) + 1);
  Size := ExpectedSize;
  SetLength(Value, Length(Expected) + 1);
  Result := (QueryProtocolValue(Handle, Name, 0, Kind, Value, Size) = 0) and
    (Kind = StringValue) and (Size = ExpectedSize) and (Value = Expected + #0);
end;

function ProtocolValueMatches(RegistryRoot: Cardinal; Key, Name, Expected: String): Boolean;
var
  Root, View, Handle: Cardinal;
begin
  ProtocolRegistryView(RegistryRoot, Root, View);
  Result := False;
  if OpenRegistryKey(Root, Key, 0, KeyQueryValue or View, Handle) <> 0 then
    Exit;
  try
    Result := ProtocolHandleValueMatches(Handle, Name, Expected);
  finally
    CloseRegistryKey(Handle);
  end;
end;

function ProtocolKeyShape(Root: Cardinal; Key, Child: String; AllowDefault, AllowMetadata: Boolean): Boolean;
var
  Names: TArrayOfString;
  Index, Status: Integer;
begin
  Status := ProtocolKeyStatus(Root, Key);
  Result := MissingEntry(Status);
  if Result or (Status <> 0) then
    Exit;
  if not RegGetValueNames(Root, Key, Names) then
    Exit;
  for Index := 0 to GetArrayLength(Names) - 1 do
    if not ((AllowDefault and (Names[Index] = '')) or (AllowMetadata and
      ((CompareText(Names[Index], ProductProtocolOwner) = 0) or
        (CompareText(Names[Index], 'URL Protocol') = 0)))) then
      Exit;
  if not RegGetSubkeyNames(Root, Key, Names) then
    Exit;
  Result := (GetArrayLength(Names) = 0) or ((GetArrayLength(Names) = 1) and
    (Child <> '') and (CompareText(Names[0], Child) = 0));
end;

function ProductProtocolCommand(Root, Version: String): String;
begin
  Result := '"' + Root + '\runtime\' + Version + '\@GUI_NAME@.exe" "%1"';
end;

function CanManageProductProtocol(RegistryRoot: Cardinal; Key, Root, Version, Previous: String): Boolean;
var
  Status: Integer;
  CommandKey: String;
begin
  Status := ProtocolKeyStatus(RegistryRoot, Key);
  Result := MissingEntry(Status);
  if Result or (Status <> 0) then
    Exit;
  CommandKey := Key + '\shell\open\command';
  // A partial write is repairable only under this instance's exact root marker.
  Result := ProtocolValueMatches(RegistryRoot, Key, ProductProtocolOwner, Root) and
    ProtocolKeyShape(RegistryRoot, Key, 'shell', True, True) and
    ProtocolKeyShape(RegistryRoot, Key + '\shell', 'open', False, False) and
    ProtocolKeyShape(RegistryRoot, Key + '\shell\open', 'command', False, False) and
    ProtocolKeyShape(RegistryRoot, CommandKey, '', True, False) and
    (not RegValueExists(RegistryRoot, Key, '') or
      ProtocolValueMatches(RegistryRoot, Key, '', ProductProtocolDescription)) and
    (not RegValueExists(RegistryRoot, Key, 'URL Protocol') or
      ProtocolValueMatches(RegistryRoot, Key, 'URL Protocol', '')) and
    (not RegValueExists(RegistryRoot, CommandKey, '') or
      ProtocolValueMatches(RegistryRoot, CommandKey, '', ProductProtocolCommand(Root, Version)) or
      ((Previous <> '') and
        ProtocolValueMatches(RegistryRoot, CommandKey, '', ProductProtocolCommand(Root, Previous))));
end;

function ClaimProductProtocol(RegistryRoot: Cardinal; Root: String): Boolean;
var
  NativeRoot, View, Handle, Disposition: Cardinal;
  Transaction: Integer;
begin
  ProtocolRegistryView(RegistryRoot, NativeRoot, View);
  Result := False;
  Transaction := BeginProtocolTransaction(0, 0, 0, 0, 0, 0, 'Claim ALCOMD link protocol');
  if Transaction = -1 then
    Exit;
  try
    if CreateProtocolKey(NativeRoot, ProductProtocolKey, 0, 0, 0, 2 or View, 0, Handle,
      Disposition, Transaction, 0) <> 0 then
      Exit;
    try
      // Before commit, a new key and its ownership marker are both private to this transaction.
      if (Disposition <> 1) or (SetProtocolOwner(Handle, ProductProtocolOwner, 0, StringValue,
        Root, 2 * (Length(Root) + 1)) <> 0) then
        Exit;
    finally
      CloseRegistryKey(Handle);
    end;
    Result := CommitProtocolTransaction(Transaction);
  finally
    if not Result then
      RollbackProtocolTransaction(Transaction);
    CloseKernelHandle(Transaction);
  end;
end;

function DeleteOwnedProtocolLeaf(RegistryRoot: Cardinal; Root: String): Boolean;
var
  NativeRoot, View, Handle, Subkeys, Values: Cardinal;
  Transaction: Integer;
begin
  ProtocolRegistryView(RegistryRoot, NativeRoot, View);
  Result := False;
  Transaction := BeginProtocolTransaction(0, 0, 0, 0, 0, 0, 'Remove ALCOMD link protocol');
  if Transaction = -1 then
    Exit;
  try
    if OpenProtocolKey(NativeRoot, ProductProtocolKey, 0, KeyQueryValue or View, Handle,
      Transaction, 0) <> 0 then
      Exit;
    try
      // Recheck the leaf inside the transaction before deleting the key and its last value.
      if (QueryProtocolKeyCounts(Handle, 0, 0, 0, Subkeys, 0, 0, Values, 0, 0, 0, 0) <> 0) or
        (Subkeys <> 0) or (Values <> 1) or
        not ProtocolHandleValueMatches(Handle, ProductProtocolOwner, Root) then
        Exit;
      if DeleteProtocolKey(NativeRoot, ProductProtocolKey, View, 0, Transaction, 0) <> 0 then
        Exit;
    finally
      CloseRegistryKey(Handle);
    end;
    Result := CommitProtocolTransaction(Transaction);
  finally
    if not Result then
      RollbackProtocolTransaction(Transaction);
    CloseKernelHandle(Transaction);
  end;
end;

function FlushProductProtocol(RegistryRoot: Cardinal; Key: String): Boolean;
var
  Root, View, Handle: Cardinal;
begin
  ProtocolRegistryView(RegistryRoot, Root, View);
  Result := False;
  if OpenRegistryKey(Root, Key, 0, KeyQueryValue or View, Handle) <> 0 then
    Exit;
  try
    Result := FlushRegistryKey(Handle) = 0;
  finally
    CloseRegistryKey(Handle);
  end;
end;

function PublishProductProtocol(Previous: String): Boolean;
var
  RegistryRoot: Cardinal;
  Root: String;
begin
  Result := False;
  if not ReadScopeRoot(IsAdminInstallMode, Root) or
    (CompareText(ExpandConstant('{app}'), Root) <> 0) or
    not PlainFileMatches(Root + '\runtime\@APP_VERSION@\@GUI_NAME@.exe', '@GUI_DIGEST@') then
    Exit;
  RegistryRoot := InstallRegistryRoot;
  if not CanManageProductProtocol(RegistryRoot, ProductProtocolKey, Root, '@APP_VERSION@', Previous) then begin
    Log('The existing @URI_SCHEME@ link protocol is not owned by this installation. It was kept.');
    Result := True;
    Exit;
  end;
  if MissingEntry(ProtocolKeyStatus(RegistryRoot, ProductProtocolKey)) and
    not ClaimProductProtocol(RegistryRoot, Root) then
    Exit;
  if not CanManageProductProtocol(RegistryRoot, ProductProtocolKey, Root, '@APP_VERSION@', Previous) then
    Exit;
  // Publish the executable command before making a new key a URL protocol handler.
  Result := RegWriteStringValue(RegistryRoot, ProductProtocolKey, '', ProductProtocolDescription) and
    RegWriteStringValue(RegistryRoot, ProductProtocolKey + '\shell\open\command', '',
      ProductProtocolCommand(Root, '@APP_VERSION@')) and
    RegWriteStringValue(RegistryRoot, ProductProtocolKey, 'URL Protocol', '') and
    FlushProductProtocol(RegistryRoot, ProductProtocolKey);
  // Bridge and scope conversion can publish after Inno's standard notification phase.
  if Result then
    NotifyProtocolChanged($08000000, 0, 0, 0); // SHCNE_ASSOCCHANGED, SHCNF_IDLIST.
end;

function DeleteProtocolValue(RegistryRoot: Cardinal; Key, Name: String): Boolean;
begin
  Result := not RegValueExists(RegistryRoot, Key, Name) or RegDeleteValue(RegistryRoot, Key, Name);
end;

function DeleteEmptyProtocolKey(RegistryRoot: Cardinal; Key: String): Boolean;
begin
  Result := MissingEntry(ProtocolKeyStatus(RegistryRoot, Key)) or RegDeleteKeyIfEmpty(RegistryRoot, Key);
end;

function RemoveOwnedProductProtocol(Root: String): Boolean;
var
  RegistryRoot: Cardinal;
  ReservedRoot, ShellKey, OpenKey, CommandKey: String;
begin
  Result := False;
  if not ReadScopeRoot(IsAdminInstallMode, ReservedRoot) or (CompareText(Root, ReservedRoot) <> 0) then
    Exit;
  Result := True;
  RegistryRoot := InstallRegistryRoot;
  if MissingEntry(ProtocolKeyStatus(RegistryRoot, ProductProtocolKey)) or
    not CanManageProductProtocol(RegistryRoot, ProductProtocolKey, ReservedRoot, '@APP_VERSION@', '') then
    Exit;
  ShellKey := ProductProtocolKey + '\shell';
  OpenKey := ShellKey + '\open';
  CommandKey := OpenKey + '\command';
  // Delete only owned values, then prune keys that are still empty.
  Result := DeleteProtocolValue(RegistryRoot, ProductProtocolKey, 'URL Protocol') and
    DeleteProtocolValue(RegistryRoot, CommandKey, '') and DeleteEmptyProtocolKey(RegistryRoot, CommandKey) and
    DeleteEmptyProtocolKey(RegistryRoot, OpenKey) and DeleteEmptyProtocolKey(RegistryRoot, ShellKey) and
    DeleteProtocolValue(RegistryRoot, ProductProtocolKey, '') and
    DeleteOwnedProtocolLeaf(RegistryRoot, ReservedRoot) and
    FlushProductProtocol(RegistryRoot, ExtractFileDir(ProductProtocolKey));
  if Result then
    NotifyProtocolChanged($08000000, 0, 0, 0);
end;
