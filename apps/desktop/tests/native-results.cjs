// Actual Windows WebView, project authority, discovery and preview; simulated Codex.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:http');
const fs = require('node:fs/promises');
const path = require('node:path');
const { tmpdir } = require('node:os');
const assert = require('node:assert/strict');
(async()=>{
 const profile=await fs.mkdtemp(path.join(tmpdir(),'forge-native-results-'));
 const project=path.join(profile,'project'); await fs.mkdir(path.join(project,'site'),{recursive:true});
 const html='<html lang="pt"><title>Jardim</title><h1>Meu jardim</h1><button onclick="this.textContent=Number(this.textContent)+1">0</button></html>';
 await fs.writeFile(path.join(project,'site/index.html'),html);
 await fs.writeFile(path.join(project,'ideias.md'),'# Meu jardim\nUm resultado para experimentar.');
 await fs.mkdir(path.join(project,'node_modules'));await fs.writeFile(path.join(project,'node_modules/oculto.md'),'excluded');
 const outside=path.join(profile,'outside');await fs.mkdir(outside);await fs.writeFile(path.join(outside,'fora.md'),'outside');
 await fs.symlink(outside,path.join(project,'atalho'),'junction');

 await fs.copyFile(path.join(__dirname,'fixtures/fake-results-server.cjs'),path.join(project,'app-server'));
 const server=createServer((req,res)=>res.writeHead(200,{'Content-Type':'text/html'}).end(html));
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const url=`http://127.0.0.1:${server.address().port}/`;
 const probe=createServer();await new Promise(resolve=>probe.listen(0,'127.0.0.1',resolve));const port=probe.address().port;await new Promise(resolve=>probe.close(resolve));
 const child=spawn(process.env.FORGE_DESKTOP_EXE,[],{windowsHide:true,stdio:'ignore',env:{...process.env,FORGE_CODEX_EXE:process.execPath,FORGE_RESULT_URL:url,WEBVIEW2_USER_DATA_FOLDER:path.join(profile,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`}});
 let browser,headless;
 try{
 for(let i=0;i<100;i++){try{browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`,{timeout:1000});break;}catch{await new Promise(r=>setTimeout(r,200));}}
 assert.ok(browser);const page=browser.contexts()[0].pages()[0];
 await page.locator('nav a[data-route="workspace"]').click();await page.locator('#custom-folder-option summary').click();await page.locator('#project-root').fill(project);
 await page.locator('#message-text').fill('Vamos criar');await page.locator('#send-message').click();
 const apps=page.locator('#preview-local-apps');await apps.waitFor({state:'visible',timeout:60000});
 const listing=await page.evaluate(projectRoot=>window.__TAURI__.core.invoke('list_project_files',{projectRoot}),project);
 assert.deepEqual(listing.files.map(f=>f.relative_path).sort(),['ideias.md','site/index.html']);
 const inspected=await page.evaluate(projectRoot=>window.__TAURI__.core.invoke('inspect_project',{projectRoot}),project);
 assert.ok(inspected.project_id, 'existing project remains readable with junction present');
 await page.locator('[data-mobile-pane-button="progress"]').click();
 await page.locator('#refresh-progress').click();
 await page.locator('#progress-result').waitFor({state:'visible',timeout:60000});
 assert.match(await page.locator('#progress-status').innerText(),/Consultado às/);
 await page.locator('[data-mobile-pane-button="preview"]').click();
 await page.locator('#result-files-search').fill('ideias');await page.locator('[data-result-path="ideias.md"]').click();
 await page.locator('#preview-path').filter({hasText:'ideias.md'}).waitFor();
 await page.locator('#message-text').fill('Meu rascunho');await page.locator('#request-preview-change').click();
 assert.ok((await page.locator('#message-text').inputValue()).startsWith('Meu rascunho\nQuero mudar o arquivo ideias.md:'));
 await page.locator('#result-files-search').fill('');await page.locator('[data-result-path="site/index.html"]').click();
 await page.locator('#preview-path').filter({hasText:/site[\\/\\\\]index\.html/}).waitFor();
 await apps.getByRole('button',{name:/Experimentar app local/}).click();await page.locator('#action-confirmation-cancel').click();
 assert.equal(await apps.locator('.local-app-card strong').textContent(),url);
 headless=await chromium.launch({headless:true});const live=await headless.newPage();await live.goto(url);await live.getByRole('button',{name:'0',exact:true}).click();assert.equal(await live.locator('button').textContent(),'1');
 await apps.getByRole('button',{name:/Pedir mudança no app local/}).click();assert.ok((await page.locator('#message-text').inputValue()).includes(`Quero mudar o resultado em ${url}:`));
 if(process.env.FORGE_RESULTS_SCREENSHOT)await page.screenshot({path:process.env.FORGE_RESULTS_SCREENSHOT,fullPage:true});
 await fs.unlink(path.join(project,'ideias.md'));await page.locator('#refresh-result-files').click();await page.waitForFunction(()=>!document.querySelector('[data-result-path="ideias.md"]'));
 assert.equal(await fs.readFile(path.join(project,'site/index.html'),'utf8'),html);
 assert.equal(await fs.readFile(path.join(outside,'fora.md'),'utf8'),'outside');
 assert.equal((await fs.lstat(path.join(project,'atalho'))).isSymbolicLink(),true);
 console.log('PASS: native onboarding and subsequent project read with pre-existing external junction; discovery excludes target/dependencies, file → preview → draft, local app confirmation/cancel + real headless interactive endpoint, refreshed deletion; external file and junction unchanged. Codex simulated; OS browser acceptance covered by browser double, not dispatched in this final native run.');
 }catch(error){
 if(browser){const p=browser.contexts()[0].pages()[0];console.log(await p.locator('body').innerText());await p.screenshot({path:'C:/ForgeFast/forge-075-failure.png',fullPage:true});}throw error;
 }finally{
 if(headless)await headless.close();if(browser){const page=browser.contexts()[0].pages()[0];await page.evaluate(()=>window.__TAURI__.core.invoke('disconnect_agent')).catch(()=>{});await browser.close();}
 child.kill();await new Promise(r=>child.exitCode!==null?r():child.once('exit',r));await new Promise(r=>server.close(r));
 await fs.unlink(path.join(project,'atalho')).catch(error=>{if(error.code!=='ENOENT')throw error;});await fs.rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:200});
 }
})().catch(error=>{console.error(error);process.exitCode=1;});
