const { createInterface } = require('node:readline');
const write = value => process.stdout.write(JSON.stringify(value)+'\n');
createInterface({input:process.stdin}).on('line', line => {
 const r=JSON.parse(line); if(!r.method)return;
 const reply=result=>write({id:r.id,result});
 switch(r.method){
 case 'initialize': reply({});break;
 case 'initialized':break;
 case 'account/read':reply({account:{type:'chatgpt'}});break;
 case 'thread/start':reply({thread:{id:'results-thread',cwd:r.params.cwd,status:{type:'idle'},turns:[]}});break;
 case 'turn/start':
 reply({turn:{id:'result-turn'}});
 write({method:'turn/started',params:{turn:{id:'result-turn'}}});
 write({method:'item/completed',params:{item:{type:'agentMessage',id:'result-message',text:`Confira [a página](site/index.html) e experimente ${process.env.FORGE_RESULT_URL}. Não publiquei nada.`}}});
 write({method:'turn/completed',params:{turn:{id:'result-turn',status:'completed'}}});break;
 default:write({id:r.id,error:{code:-32601,message:'Unsupported fixture method'}});
 }
});
