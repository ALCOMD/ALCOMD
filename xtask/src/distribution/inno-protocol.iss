type
  TProtocolState = (ProtocolMissing, ProtocolOwned, ProtocolForeign, ProtocolFailed);

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

function SetProtocolValue(Handle: Cardinal; Name: String; Reserved, ValueType: Cardinal;
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

function ProtocolValueStatus(Handle: Cardinal; Name, Expected: String): Integer;
var
  Kind, Size, ExpectedSize: Cardinal;
  Value: String;
begin
  // Check the exact type, bytes, and single UTF-16 terminator.
  ExpectedSize := 2 * (Length(Expected) + 1);
  Size := ExpectedSize;
  SetLength(Value, Length(Expected) + 1);
  Result := QueryProtocolValue(Handle, Name, 0, Kind, Value, Size);
  if (Result = 0) and ((Kind <> StringValue) or (Size <> ExpectedSize) or
    (Value <> Expected + #0)) then
    Result := 13; // ERROR_INVALID_DATA.
end;

function CheckProtocolValue(Handle: Cardinal; Name, Expected, Previous: String;
  Required: Boolean; var KnownValues: Cardinal): TProtocolState;
var
  Status: Integer;
begin
  Status := ProtocolValueStatus(Handle, Name, Expected);
  if ((Status = 13) or (Status = 234)) and (Previous <> '') then
    Status := ProtocolValueStatus(Handle, Name, Previous);
  Result := ProtocolFailed;
  if Status = 0 then begin
    KnownValues := KnownValues + 1;
    Result := ProtocolOwned;
  end else if MissingEntry(Status) and not Required then
    Result := ProtocolOwned
  else if MissingEntry(Status) or (Status = 13) or (Status = 234) then
    Result := ProtocolForeign;
end;

function ProductProtocolCommand(Root, Version: String): String;
begin
  Result := '"' + Root + '\runtime\' + Version + '\@GUI_NAME@.exe" "%1"';
end;

function ProtocolKeyAtDepth(Key: String; Depth: Integer): String;
begin
  case Depth of
    0: Result := Key;
    1: Result := Key + '\shell';
    2: Result := Key + '\shell\open';
    3: Result := Key + '\shell\open\command';
  end;
end;

function InspectProtocolNode(NativeRoot, View: Cardinal; Key, Root, Command, PriorCommand: String;
  Depth, Transaction: Integer; var LastDepth: Integer): TProtocolState;
var
  Handle, Subkeys, Values, KnownValues: Cardinal;
  Status: Integer;
  NodeKey: String;
begin
  // The cross-scope preflight is only a read. Mutations always use transacted handles here.
  NodeKey := ProtocolKeyAtDepth(Key, Depth);
  if Transaction = 0 then
    Status := OpenRegistryKey(NativeRoot, NodeKey, 0, KeyQueryValue or View, Handle)
  else
    Status := OpenProtocolKey(NativeRoot, NodeKey, 0, KeyQueryValue or View, Handle, Transaction, 0);
  Result := ProtocolFailed;
  if MissingEntry(Status) then begin
    Result := ProtocolMissing;
    Exit;
  end;
  if Status <> 0 then
    Exit;
  try
    if QueryProtocolKeyCounts(Handle, 0, 0, 0, Subkeys, 0, 0, Values, 0, 0, 0, 0) <> 0 then
      Exit;
    KnownValues := 0;
    Result := ProtocolOwned;
    if Depth = 0 then begin
      Result := CheckProtocolValue(Handle, ProductProtocolOwner, Root, '', True, KnownValues);
      if Result = ProtocolOwned then
        Result := CheckProtocolValue(Handle, '', ProductProtocolDescription, '', False, KnownValues);
      if Result = ProtocolOwned then
        Result := CheckProtocolValue(Handle, 'URL Protocol', '', '', False, KnownValues);
    end else if Depth = 3 then
      Result := CheckProtocolValue(Handle, '', Command, PriorCommand, False, KnownValues);
    if Result <> ProtocolOwned then
      Exit;
    // The exact known values account for the entire node, without enumerating unknown names.
    if (Values <> KnownValues) or ((Depth = 3) and (Subkeys <> 0)) or (Subkeys > 1) then begin
      Result := ProtocolForeign;
      Exit;
    end;
    LastDepth := Depth;
    if Subkeys = 1 then begin
      Result := InspectProtocolNode(NativeRoot, View, Key, Root, Command, PriorCommand,
        Depth + 1, Transaction, LastDepth);
      if Result = ProtocolMissing then
        Result := ProtocolForeign;
    end;
  finally
    CloseRegistryKey(Handle);
  end;
end;

function InspectProductProtocol(NativeRoot, View: Cardinal; Key, Root, Version, Previous: String;
  Transaction: Integer; var LastDepth: Integer): TProtocolState;
var
  PreviousCommand: String;
begin
  LastDepth := -1;
  PreviousCommand := '';
  if Previous <> '' then
    PreviousCommand := ProductProtocolCommand(Root, Previous);
  Result := InspectProtocolNode(NativeRoot, View, Key, Root, ProductProtocolCommand(Root, Version),
    PreviousCommand, 0, Transaction, LastDepth);
end;

function CanManageProductProtocol(RegistryRoot: Cardinal; Key, Root, Version, Previous: String): Boolean;
var
  NativeRoot, View: Cardinal;
  State: TProtocolState;
  LastDepth: Integer;
begin
  ProtocolRegistryView(RegistryRoot, NativeRoot, View);
  State := InspectProductProtocol(NativeRoot, View, Key, Root, Version, Previous, 0, LastDepth);
  Result := (State = ProtocolOwned) or (State = ProtocolMissing);
end;

function WriteProtocolValue(Handle: Cardinal; Name, Value: String): Boolean;
begin
  Result := SetProtocolValue(Handle, Name, 0, StringValue, Value + #0, 2 * (Length(Value) + 1)) = 0;
end;

function OpenProtocolTreeForWrite(NativeRoot, View: Cardinal; Depth, Transaction, LastDepth: Integer;
  var Handle: Cardinal): Boolean;
var
  Access, Disposition: Cardinal;
  Key: String;
begin
  Access := KeyQueryValue;
  if (Depth = 0) or (Depth = 3) then
    Access := 2; // KEY_SET_VALUE.
  Key := ProtocolKeyAtDepth(ProductProtocolKey, Depth);
  if Depth <= LastDepth then begin
    Result := OpenProtocolKey(NativeRoot, Key, 0, Access or View, Handle, Transaction, 0) = 0;
    Exit;
  end;
  Result := False;
  if CreateProtocolKey(NativeRoot, Key, 0, 0, 0, Access or View, 0, Handle,
    Disposition, Transaction, 0) <> 0 then
    Exit;
  // A newly appeared key cannot become an owned claim, even if its name is expected.
  Result := Disposition = 1;
  if not Result then
    CloseRegistryKey(Handle);
end;

function WriteProtocolTree(NativeRoot, View: Cardinal; Root: String; Transaction, LastDepth: Integer): Boolean;
var
  Handle, ChildHandle: Cardinal;
  Depth: Integer;
begin
  Result := False;
  if not OpenProtocolTreeForWrite(NativeRoot, View, 0, Transaction, LastDepth, Handle) then
    Exit;
  try
    for Depth := 1 to 3 do begin
      if not OpenProtocolTreeForWrite(NativeRoot, View, Depth, Transaction, LastDepth, ChildHandle) then
        Exit;
      try
        if (Depth = 3) and
          not WriteProtocolValue(ChildHandle, '', ProductProtocolCommand(Root, '@APP_VERSION@')) then
          Exit;
      finally
        CloseRegistryKey(ChildHandle);
      end;
    end;
    Result := ((LastDepth >= 0) or WriteProtocolValue(Handle, ProductProtocolOwner, Root)) and
      WriteProtocolValue(Handle, '', ProductProtocolDescription) and WriteProtocolValue(Handle, 'URL Protocol', '');
  finally
    CloseRegistryKey(Handle);
  end;
end;

function DeleteProtocolTree(NativeRoot, View: Cardinal; Transaction, LastDepth: Integer): Boolean;
var
  Depth: Integer;
begin
  Result := False;
  // Delete only inspected nodes, leaf first. The native API refuses keys with subkeys.
  for Depth := LastDepth downto 0 do
    if DeleteProtocolKey(NativeRoot, ProtocolKeyAtDepth(ProductProtocolKey, Depth), View, 0,
      Transaction, 0) <> 0 then
      Exit;
  Result := True;
end;

function FlushProductProtocol(NativeRoot, View: Cardinal; Key: String): Boolean;
var
  Handle: Cardinal;
begin
  Result := False;
  if OpenRegistryKey(NativeRoot, Key, 0, KeyQueryValue or View, Handle) <> 0 then
    Exit;
  try
    Result := FlushRegistryKey(Handle) = 0;
  finally
    CloseRegistryKey(Handle);
  end;
end;

function ChangeProductProtocol(Root, Previous: String; Removing, RequireOwnership: Boolean): Boolean;
var
  NativeRoot, View: Cardinal;
  State, PublicState: TProtocolState;
  LastDepth, PublicDepth, Transaction: Integer;
  Mutated, Committed: Boolean;
begin
  ProtocolRegistryView(InstallRegistryRoot, NativeRoot, View);
  Result := False;
  Committed := False;
  Transaction := BeginProtocolTransaction(0, 0, 0, 0, 0, 0, 'Maintain ALCOMD link protocol');
  if Transaction = -1 then
    Exit;
  try
    State := InspectProductProtocol(NativeRoot, View, ProductProtocolKey, Root, '@APP_VERSION@',
      Previous, Transaction, LastDepth);
    if State = ProtocolFailed then
      Exit;
    if State = ProtocolForeign then begin
      Log('The existing @URI_SCHEME@ link protocol is not owned by this installation. It was kept.');
      Result := not RequireOwnership;
      Exit;
    end;
    if Removing and (State = ProtocolMissing) then begin
      Result := True;
      Exit;
    end;
    if Removing then
      Mutated := DeleteProtocolTree(NativeRoot, View, Transaction, LastDepth)
    else
      Mutated := WriteProtocolTree(NativeRoot, View, Root, Transaction, LastDepth);
    if not Mutated then
      Exit;
    // Reading a transacted key does not prevent a competing write before we stage changes.
    // Check the still-public tree after staging, while the touched keys are enlisted.
    PublicState := InspectProductProtocol(NativeRoot, View, ProductProtocolKey, Root,
      '@APP_VERSION@', Previous, 0, PublicDepth);
    if PublicState = ProtocolForeign then begin
      Log('The existing @URI_SCHEME@ link protocol changed during this operation. It was kept.');
      Result := not RequireOwnership;
      Exit;
    end;
    if (PublicState <> State) or (PublicDepth <> LastDepth) then
      Exit;
    Committed := CommitProtocolTransaction(Transaction);
  finally
    if not Committed then
      RollbackProtocolTransaction(Transaction);
    CloseKernelHandle(Transaction);
  end;
  Result := Committed and FlushProductProtocol(NativeRoot, View, ExtractFileDir(ProductProtocolKey));
  // Bridge and scope conversion can publish after Inno's standard notification phase.
  if Result then
    NotifyProtocolChanged($08000000, 0, 0, 0); // SHCNE_ASSOCCHANGED, SHCNF_IDLIST.
end;

function PublishProductProtocol(Previous: String; RequireOwnership: Boolean): Boolean;
var
  Root: String;
begin
  Result := ReadScopeRoot(IsAdminInstallMode, Root) and
    (CompareText(ExpandConstant('{app}'), Root) = 0) and
    PlainFileMatches(Root + '\runtime\@APP_VERSION@\@GUI_NAME@.exe', '@GUI_DIGEST@') and
    ChangeProductProtocol(Root, Previous, False, RequireOwnership);
end;

function RemoveOwnedProductProtocol(Root: String; RequireOwnership: Boolean): Boolean;
var
  ReservedRoot: String;
begin
  Result := ReadScopeRoot(IsAdminInstallMode, ReservedRoot) and (CompareText(Root, ReservedRoot) = 0) and
    ChangeProductProtocol(ReservedRoot, '', True, RequireOwnership);
end;
