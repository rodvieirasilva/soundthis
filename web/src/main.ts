import "./style.css";
import { SoundThisApp } from "./ui/app";

window.addEventListener("DOMContentLoaded", () => {
  const app = new SoundThisApp();
  app.init();
});
