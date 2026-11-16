const {chromium}=require(process.env.PLAYWRIGHT_CORE || 'playwright-core');
const assert=require('assert');
(async()=>{
  const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH||undefined,args:['--no-sandbox'],headless:true});
  const page=await browser.newPage();
  await page.goto(process.env.ISA_TEST_URL||'http://127.0.0.1:4174/');
  await page.evaluate(()=>localStorage.setItem('isa.customProviders.v1',JSON.stringify(['a','b'].map(name=>({id:'custom-'+name,name,model:'m',url:'https://'+name+'.example.com/v1/chat/completions'})))));
  await page.reload();
  let calls=[];
  await page.route('**/api/chat',r=>r.fulfill({status:503,contentType:'application/json',body:JSON.stringify({provider:'decoy',role:'assistant',error:{message:'No provider'}})}));
  await page.route('https://*.example.com/**',r=>{calls.push(r.request().url());return r.fulfill({status:400,headers:{'Access-Control-Allow-Origin':'*'},body:'invalid'})});
  await page.locator('#prompt').fill('hello');await page.locator('#send').click();await page.getByRole('dialog').getByRole('button',{name:'Send',exact:true}).click();
  await page.getByText(/Auto stopped at a/).waitFor();assert.equal(calls.length,1,'HTTP 400 must not send the request to another provider');
  await page.unroute('https://*.example.com/**');
  await page.route('https://*.example.com/**',async r=>{await new Promise(resolve=>setTimeout(resolve,1500));try{await r.fulfill({status:200,contentType:'application/json',body:JSON.stringify({choices:[{message:{content:'late'}}]})})}catch{}});
  await page.locator('#prompt').fill('stop this');await page.locator('#send').click();await page.getByRole('dialog').getByRole('button',{name:'Send',exact:true}).click();await page.getByRole('button',{name:'Stop response'}).click();await page.getByText('Request stopped.').waitFor();
  await browser.close();console.log('Auto non-retryable failure and cancellation passed');
})().catch(error=>{console.error(error);process.exit(1)});
