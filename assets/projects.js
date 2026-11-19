// Compatibility facade for classic callers. Project UI behavior lives in frontend/src/project-ui.mjs.
let projectUI;
function getProjectUI(){
  if(!projectUI)projectUI=isaProjectUI.createProjectUI({
    document, get sessionStorage(){return sessionStorage},
    get projectStore(){return projectStore},get chats(){return chats},
    get projectsSection(){return projectsSection},get projectSearch(){return projectSearch},get projectGrid(){return projectGrid},get projectDialog(){return projectDialog},
    get welcome(){return welcome},get thread(){return thread},get region(){return region},
    set activeChatId(value){activeChatId=value},
    isaNotifications,isaSidebar,node:(...args)=>node(...args),renderRecent:(...args)=>renderRecent(...args),
    setActiveSection:(...args)=>setActiveSection(...args),newChat:(...args)=>newChat(...args),openChat:(...args)=>openChat(...args),makeChatAvatar:(...args)=>makeChatAvatar(...args),
  });
  return projectUI;
}
function refreshProjectViews(...args){return getProjectUI().refreshProjectViews(...args)}
function projectChange(...args){return getProjectUI().projectChange(...args)}
function clearChatDrag(...args){return getProjectUI().clearChatDrag(...args)}
function enableChatDrag(...args){return getProjectUI().enableChatDrag(...args)}
function enableProjectDrop(...args){return getProjectUI().enableProjectDrop(...args)}
function renderSidebarProjects(...args){return getProjectUI().renderSidebarProjects(...args)}
function projectChats(...args){return getProjectUI().projectChats(...args)}
function updateProject(...args){return getProjectUI().updateProject(...args)}
function renderProjects(...args){return getProjectUI().renderProjects(...args)}
function showProjects(...args){return getProjectUI().showProjects(...args)}
function showProject(...args){return getProjectUI().showProject(...args)}
function openProjectDialog(...args){return getProjectUI().openProjectDialog(...args)}
