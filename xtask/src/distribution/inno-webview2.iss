function WebView2VersionPresent(Value: String): Boolean;
var
  Position, Parts: Integer;
  InNumber, Positive: Boolean;
begin
  Result := False;
  InNumber := False;
  Positive := False;
  Parts := 1;
  for Position := 1 to Length(Value) do begin
    if Value[Position] = '.' then begin
      if not InNumber or (Parts = 4) then
        Exit;
      Parts := Parts + 1;
      InNumber := False;
    end else begin
      if Pos(Value[Position], '0123456789') = 0 then
        Exit;
      InNumber := True;
      Positive := Positive or (Value[Position] <> '0');
    end;
  end;
  Result := InNumber and (Parts = 4) and Positive;
end;

function WebView2Available: Boolean;
var
  Key, Version: String;
begin
  Key := 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  Result := RegQueryStringValue(HKEY_LOCAL_MACHINE_32, Key, 'pv', Version) and
    WebView2VersionPresent(Version);
  if not Result and not IsAdminInstallMode then
    Result := RegQueryStringValue(HKEY_CURRENT_USER, Key, 'pv', Version) and
      WebView2VersionPresent(Version);
end;

function EnsureWebView2(var NeedsRestart: Boolean): String;
var
  Installer, Digest, Executable, WorkingDirectory: String;
  Started: Boolean;
  InstallerExitCode: Integer;
begin
  Result := '';
  if WebView2Available then
    Exit;
  // ExecAsOriginalUser cannot de-elevate an already-elevated setup process.
  if not IsAdminInstallMode and IsAdmin then begin
    Result := 'For a current-user installation, start ALCOMD Setup without administrative privileges and retry.';
    Exit;
  end;

  Installer := '@WEBVIEW2_BOOTSTRAPPER_NAME@';
  Digest := '@WEBVIEW2_BOOTSTRAPPER_DIGEST@';
  if '@WEBVIEW2_STANDALONE_DIGEST@' <> '' then begin
    Installer := '@WEBVIEW2_STANDALONE_NAME@';
    Digest := '@WEBVIEW2_STANDALONE_DIGEST@';
  end;
  WorkingDirectory := ExpandConstant('{tmp}');
  Executable := AddBackslash(WorkingDirectory) + Installer;
  Result := 'The bundled Microsoft Edge WebView2 installer could not be verified.';
  try
    ExtractTemporaryFile(Installer);
    if not PlainFile(Executable) or (CompareText(GetSHA256OfFile(Executable), Digest) <> 0) then
      Exit;
  except
    Log('Could not extract or verify the bundled Microsoft Edge WebView2 installer.');
    Exit;
  end;

  Result := 'Microsoft Edge WebView2 Evergreen Runtime is required. Install it and retry ALCOMD Setup.';
  if IsAdminInstallMode then
    Started := Exec(Executable, '/silent /install', WorkingDirectory, SW_HIDE,
      ewWaitUntilTerminated, InstallerExitCode)
  else
    Started := ExecAsOriginalUser(Executable, '/silent /install', WorkingDirectory, SW_HIDE,
      ewWaitUntilTerminated, InstallerExitCode);
  if not Started then begin
    Log('Could not start the Microsoft Edge WebView2 installer.');
    Exit;
  end;
  Log('Microsoft Edge WebView2 installer result: ' + IntToStr(InstallerExitCode));
  if InstallerExitCode = 3010 then begin
    NeedsRestart := True;
    Result := 'Restart Windows to finish WebView2 installation, then retry ALCOMD Setup.';
  end else if (InstallerExitCode = 0) and WebView2Available then
    Result := '';
end;
