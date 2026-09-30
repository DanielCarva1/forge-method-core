// Actual Windows save dialog + native project/file authority and byte copy.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { once } = require('node:events');
const { createServer } = require('node:net');
const fs = require('node:fs/promises');
const path = require('node:path');
const { tmpdir } = require('node:os');
const assert = require('node:assert/strict');
async function saveDialog(page, mode, destination) {
 const helper=spawn('py',['-3.12',path.join(__dirname,'folder-dialog.py'),mode,...(destination?[destination]:[])],{windowsHide:true});
 let output='';helper.stdout.on('data',chunk=>output+=chunk);helper.stderr.on('data',chunk=>output+=chunk);
 const finished=once(helper,'exit');await page.locator('#save-result-copy').click();const [code]=await finished;assert.equal(code,0,output);
}
(async()=>{
 const profile=await fs.mkdtemp(path.join(tmpdir(),'forge-native-copy-'));const project=path.join(profile,'project');await fs.mkdir(project);
 const source=path.join(project,'Ideia com espaço.md');const content='# Meu jardim\nMeu resultado, sem caminhos complicados.\n';await fs.writeFile(source,content);
 const destination=path.join(profile,'Cópia da ideia.md');
 const large=path.join(project,'Página grande.html');const largeContent='<h1>Meu jardim</h1><!--'+'x'.repeat(513*1024)+'-->';await fs.writeFile(large,largeContent);
 const reservation=createServer();await new Promise(r=>reservation.listen(0,'127.0.0.1',r));const port=reservation.address().port;await new Promise(r=>reservation.close(r));
 const child=spawn(process.env.FORGE_DESKTOP_EXE,[],{windowsHide:true,stdio:'ignore',env:{...process.env,WEBVIEW2_USER_DATA_FOLDER:path.join(profile,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`}});
 let browser;
 try{
 for(let i=0;i<100;i++){try{browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`,{timeout:1000});break;}catch{await new Promise(r=>setTimeout(r,200));}}
 assert.ok(browser);const page=browser.contexts()[0].pages()[0];
 await page.locator('nav a[data-route="workspace"]').click();await page.locator('#custom-folder-option summary').click();await page.locator('#project-root').fill(project);await page.locator('#start-project').click();
 await page.locator('[data-result-path="Ideia com espaço.md"]').click();await page.locator('#preview-path').filter({hasText:'Ideia com espaço.md'}).waitFor();
 await page.locator('#message-text').fill('Rascunho que não deve ser enviado');
 await saveDialog(page,'save-cancel');await page.locator('#preview-status').filter({hasText:'Você cancelou'}).waitFor();
 await assert.rejects(fs.access(destination));assert.equal(await fs.readFile(source,'utf8'),content);
 await saveDialog(page,'save-select',destination);await page.locator('#preview-status').filter({hasText:'Cópia salva'}).waitFor();
 assert.equal(await fs.readFile(destination,'utf8'),content);assert.equal(await fs.readFile(source,'utf8'),content);
 assert.equal(await page.locator('#message-text').inputValue(),'Rascunho que não deve ser enviado');
 // Avoid an Explorer window on the active user's desktop. Other calls still use actual IPC.
 await page.evaluate(()=>{const core=window.__TAURI__.core;const facade=Object.create(core);window.revealed=[];Object.defineProperty(facade,'invoke',{value:(command,args)=>command==='reveal_project_file'?(window.revealed.push(args),Promise.resolve()):core.invoke(command,args)});window.__TAURI__.core=facade;});
 await page.locator('#reveal-result-file').click();await page.waitForFunction(()=>window.revealed.length===1);
 assert.equal(await page.evaluate(()=>window.revealed[0].projectRoot),project);
 assert.ok((await page.evaluate(()=>window.revealed[0].filePath)).endsWith('Ideia com espaço.md'));
 await page.locator('[data-result-path="Página grande.html"]').click();await page.locator('#preview-file-note').waitFor({state:'visible'});assert.equal(await page.locator('#preview-site').isVisible(),false);
 const largeCopy=path.join(profile,'Cópia da página.html');await saveDialog(page,'save-select',largeCopy);await page.locator('#preview-status').filter({hasText:'Cópia salva'}).waitFor();assert.equal(await fs.readFile(largeCopy,'utf8'),largeContent);assert.equal(await fs.readFile(large,'utf8'),largeContent);
 if(process.env.FORGE_COPY_SCREENSHOT){await page.evaluate(()=>window.scrollTo(0,0));await page.screenshot({path:process.env.FORGE_COPY_SCREENSHOT,fullPage:true});}
 console.log('PASS: native project → file preview → actual Save dialog cancellation/selection → exact copied bytes; original and draft untouched. Explorer dispatch mocked; no Codex/model used.');
 }catch(error){if(browser){const p=browser.contexts()[0].pages()[0];console.log(await p.locator('body').innerText());}throw error;}
 finally{if(browser)await browser.close();child.kill();await new Promise(r=>child.exitCode!==null?r():child.once('exit',r));await fs.rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:200});}
})().catch(error=>{console.error(error);process.exitCode=1;});
