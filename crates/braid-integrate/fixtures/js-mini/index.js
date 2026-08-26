import axios from "axios";
import { v4 as uuidv4 } from "uuid";
import cron from "node-cron";
cron.schedule("* * * * *", () => {
  fetch("https://example.com").then(r => r.json());
  console.log(uuidv4());
});
