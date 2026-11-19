// Test-only MCP stdio server: no external network or dependencies.
const readline=require('node:readline'),fs=require('node:fs');
const mode=process.argv[2]||'valid',marker=process.argv[3];
if(marker)fs.writeFileSync(marker+'.started','started');
let initialized=false,pendingPing=null,calls=0;
function send(v){process.stdout.write(JSON.stringify(v)+'\n')}
readline.createInterface({input:process.stdin,crlfDelay:Infinity}).on('line',line=>{
 const request=JSON.parse(line);
 if(pendingPing && request.id==='server-ping' && !request.method){
  if(JSON.stringify(request.result)!=='{}')throw Error('bad ping response');
  send({jsonrpc:'2.0',id:pendingPing,result:{tools:[{name:'echo',inputSchema:{type:'object'}}]}});pendingPing=null;return;
 }
 if(request.method==='notifications/initialized'){initialized=true;return}
 if(mode==='hang')return;
 if(mode==='invalid'){process.stdout.write('not json\n');return}
 if(mode==='oversized'){process.stdout.write('x'.repeat(140000)+'\n');return}
 if(request.method==='initialize')return send({jsonrpc:'2.0',id:mode==='wrongid'?99:request.id,result:{protocolVersion:mode==='version'?'2024-11-05':'2025-06-18',capabilities:{tools:{}},serverInfo:{name:'fixture',version:'1'}}});
 if(!initialized)throw Error('not initialized');
 if(request.method==='tools/list'){
  if(mode==='ping'){pendingPing=request.id;return send({jsonrpc:'2.0',id:'server-ping',method:'ping'});}
  if(mode==='cursor')return send({jsonrpc:'2.0',id:request.id,result:{tools:[],nextCursor:'repeat'}});
  if(!request.params.cursor)return send({jsonrpc:'2.0',method:'notifications/message',params:{level:'info',data:'untrusted'}}),send({jsonrpc:'2.0',id:request.id,result:{tools:[{name:'echo',description:'echo test',inputSchema:mode==='schema'?{type:'object',properties:{value:{type:'string',minLength:1}},required:['value'],additionalProperties:false}:{type:'object'},...(mode==='retry'?{annotations:{idempotentHint:true}}:{})}],nextCursor:'page2'}});
  return send({jsonrpc:'2.0',id:request.id,result:{tools:[{name:'write',description:'writes a marker',inputSchema:{type:'object'}}]}});
 }
 if(request.method==='tools/call'){
  calls++;if(marker){fs.writeFileSync(marker,JSON.stringify(request.params));fs.writeFileSync(marker+'.calls',String(calls));}
  return send({jsonrpc:'2.0',id:request.id,result:{content:[{type:'text',text:'fixture result'}],isError:mode==='toolerror'||(mode==='retry'&&calls===1)}});
 }
});
