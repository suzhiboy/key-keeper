# Internal interface contract

Tauri command `rpc(request: serde_json::Value) -> Result<Value,String>`. Frontend invoke('rpc',{request:{op,...args,session}}). Errors are concise Chinese strings, never secrets. Authenticated calls require the current opaque session; frontend also discards results after local session changes. Event `vault-locked` clears UI state. Poll status also detects locks.

Record = {id:string,kind:'login'|'api'|'totp'|'card',title:string,tags:string[],notes:string,fields:Record<string,string>,used:boolean,createdAt:string,updatedAt:string}. Blank title derived by backend. All fields optional, even invalid/incomplete TOTP may save; validate only when generating codes. fields keys: platform,username,email,phone,password,website,provider,endpoint,secret,issuer,accountName,totpSecret,totpAlgorithm,totpDigits,totpPeriod,cardNumber,secretCode,expiryDate. Secret keys=password,secret,totpSecret,secretCode. TOTP uses totpSecret (Base32 or otpauth URI), defaults SHA1/6/30. Login and totp support TOTP. Lists/details normally redact secret values, return `secretFields:string[]` to identify nonempty values.

Operations (args => result):
- status {} => {exists:boolean,unlocked:boolean,session:string|null}
- create/unlock {password} => status
- lock {} => null; activity {session} => null (only actual input events)
- list {session,query?,kind?,tag?} => Record[] (redacted)
- detail {session,id,includeSecrets?:boolean} => Record
- save {session,record} => redacted Record; blank id means create
- delete {session,id} => null
- reveal {session,id,field} => string
- copy {session,id,field} => null; field='totp' generates fresh code
- totp {session,id} => {code:string,remaining:number,period:number}
- change_password {session,oldPassword,newPassword} => status (new session)
- export_backup {session} => {cancelled:boolean} using native save dialog
- inspect_backup {} => {cancelled:boolean,backupId?:string} using native open dialog
- restore_preview {backupId,password} => {count:number} validates backup, retains decrypted candidate internally
- restore_backup {backupId} => status, atomic replacement then locked (UI must confirm first)
- import_open {session} => {cancelled:boolean,importId?:string,fileName?:string,sheets?:string[]}
- import_preview {session,importId,kind,encoding:'utf8'|'gb18030',delimiter:string,sheet?:string,hasHeader:boolean,mapping:Record<string,number>,skipDuplicates:boolean} => {columns:string[],sampleRows:string[][],rows:{index:number,title:string,status:'valid'|'duplicate'|'error',message:string}[],validCount:number,errorCount:number,duplicateCount:number}; mapping keys title,tags,notes or field keys; index is zero-based column. Empty mapping returns column/sample preview with no commit-ready rows. TXT default delimiter blank means single column; UI defaults map column0 to secretCode for card, totpSecret for totp; other kinds user maps. Sample secret columns mask once mapped. Corrections via mapping/source, not inline row editing.
- import_commit {session,importId,skipErrors:boolean} => {imported:number,skipped:number}; requires preceding successful preview, no overwrites, whole batch atomic
- import_cancel {session,importId} => null

Windows persistence default app_data_dir/vault.kkv. Test override `KEY_KEEPER_DATA_DIR` only respected in debug builds. No frontend filesystem plugin. UI theme localStorage only, no secret persistence. Closing main window exits; no tray.
