const {chromium}=require(process.env.PLAYWRIGHT_CORE || 'playwright-core'),assert=require('assert');
(async()=>{
  const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH||undefined,args:['--no-sandbox'],headless:true}),page=await browser.newPage({viewport:{width:1300,height:900}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await page.goto(process.env.ISA_TEST_URL||'http://127.0.0.1:4174/');
  await page.evaluate(()=>{
    const names=['a','b','c','d'];localStorage.setItem('isa.customProviders.v1',JSON.stringify(names.map(name=>({id:'custom-'+name,name,model:'m',url:'https://'+name+'.example.com/v1/chat/completions'}))));
    const nodes=names.map((name,i)=>({id:'custom-'+name,providerId:'custom-'+name,type:'agent',x:i*180,y:100}));
    const edges=[{from:'custom-a',to:'custom-b',type:'manage'},{from:'custom-b',to:'custom-c',type:'manage'},{from:'custom-a',to:'custom-d',type:'manage'},{from:'custom-d',to:'custom-c',type:'manage'}];
    localStorage.setItem('isa.agentPools.v1',JSON.stringify([{id:'p',name:'Team',members:names.map(n=>'custom-'+n),edges:[],graph:{nodes,edges}}]));
  });await page.reload();
  let requests=0;await page.route('https://*.example.com/**',r=>{requests++;return r.fulfill({status:200,contentType:'application/json',body:JSON.stringify({choices:[{message:{content:'not valid JSON'}}]})})});
  await page.locator('#routingButton').click();const trigger=page.getByRole('menuitem',{name:'Agent pools'});await trigger.focus();await page.keyboard.press('ArrowRight');let submenu=page.getByRole('menu',{name:'Agent pools'});assert(await submenu.isVisible());const mainBox=await page.locator('#routingMenu').boundingBox(),subBox=await submenu.boundingBox();assert(subBox.x>=mainBox.x+mainBox.width-1,'submenu should open to the right when there is room');await page.keyboard.press('Escape');assert(!(await submenu.isVisible()));await trigger.click();await submenu.getByRole('menuitemradio',{name:/Team/}).click();
  await page.locator('#prompt').fill('Do work');await page.locator('#send').click();await page.getByRole('dialog').getByRole('button',{name:'Send',exact:true}).click();await page.getByText(/only one manager/).waitFor();assert.equal(requests,0,'ambiguous workflow must not make calls');
  await page.evaluate(()=>{const list=JSON.parse(localStorage.getItem('isa.agentPools.v1'));list[0].graph.edges=list[0].graph.edges.slice(0,1);list[0].graph.nodes=list[0].graph.nodes.slice(0,2);localStorage.setItem('isa.agentPools.v1',JSON.stringify(list))});await page.reload();
  await page.locator('#prompt').fill('Delegate');await page.locator('#send').click();await page.getByRole('dialog').getByRole('button',{name:'Send',exact:true}).click();await page.getByText(/did not return a valid delegation plan/).waitFor();assert.equal(requests,1,'invalid plan must not launch a child');
  await page.setViewportSize({width:375,height:800});await page.locator('#routingButton').click();await page.getByRole('menuitem',{name:'Agent pools'}).click();assert(await page.getByRole('menu',{name:'Agent pools'}).isVisible());await page.getByRole('menu',{name:'Agent pools'}).getByRole('menuitemradio',{name:/Team/}).click();
  assert.deepEqual(errors,[]);await browser.close();console.log('Pool submenu, keyboard, responsive layout and invalid delegation safeguards passed');
})().catch(error=>{console.error(error);process.exit(1)});
