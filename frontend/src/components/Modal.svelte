<script>
  import {onMount} from 'svelte';
  import {X} from 'lucide-svelte';
  export let title;export let onclose=()=>{};export let wide=false;
  let dialog;
  onMount(()=>{dialog.showModal();return()=>dialog?.close();});
</script>
<!-- svelte-ignore a11y_no_noninteractive_element_interactions (native dialog backdrop dismissal) -->
<dialog bind:this={dialog} aria-label={title} class:wide oncancel={(e)=>{e.preventDefault();onclose();}} onclick={e=>{if(e.target===dialog)onclose();}}>
  <div class="modal-inner"><div class="modal-heading"><h2>{title}</h2><button class="icon-button" onclick={onclose} aria-label="Close dialog"><X size={19}/></button></div><slot/></div>
</dialog>
