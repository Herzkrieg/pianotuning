const plannerForm = document.getElementById("planner-form");
const monthsInput = document.getElementById("months");
const monthsValue = document.getElementById("months-value");
const recommendation = document.getElementById("recommendation");
const summaryCopy = document.getElementById("summary-copy");
const estimate = document.getElementById("estimate");
const duration = document.getElementById("duration");
const serviceArea = document.getElementById("service-area");
const keyboard = document.getElementById("keyboard");
const keyboardTip = document.getElementById("keyboard-tip");
const contactForm = document.getElementById("contact-form");
const confirmationMessage = document.getElementById("confirmation-message");

function updatePlanner() {
  const formData = new FormData(plannerForm);
  const months = Number(formData.get("months"));
  const repair = formData.has("repair");
  const priority = formData.has("priority");
  const pianoType = formData.get("pianoType");
  const area = formData.get("area");

  let title = "Standard tuning visit";
  let copy = "Great for a piano that has been serviced within the last year and needs a refresh.";
  let price = 145;
  let time = "60–75 min";

  if (months > 12) {
    title = "Pitch raise + tuning visit";
    copy = "Your piano will likely benefit from a longer visit to restore stable tuning across the scale.";
    price = 220;
    time = "90–120 min";
  }

  if (repair) {
    title = months > 12 ? "Pitch raise, tuning & support visit" : "Tuning & support visit";
    copy = "A combined visit gives us time to tune the piano and inspect common action or pedal concerns.";
    price += 40;
    time = months > 12 ? "110–140 min" : "75–95 min";
  }

  if (priority) {
    price += 25;
  }

  if (pianoType === "Grand") {
    price += 20;
  }

  if (pianoType === "Digital + acoustic support") {
    copy = "We can help with acoustic care questions and setup guidance for hybrid or digital practice spaces.";
    price -= 10;
    time = repair ? "60–80 min" : "45–60 min";
  }

  monthsValue.textContent = String(months);
  recommendation.textContent = title;
  summaryCopy.textContent = copy;
  estimate.textContent = `$${price}`;
  duration.textContent = time;
  serviceArea.textContent = area;
}

plannerForm.addEventListener("input", updatePlanner);
updatePlanner();

keyboard.addEventListener("click", (event) => {
  const key = event.target.closest(".key");
  if (!key) {
    return;
  }

  keyboardTip.textContent = key.dataset.tip;
});

contactForm.addEventListener("submit", (event) => {
  event.preventDefault();

  const formData = new FormData(contactForm);
  const name = formData.get("name");
  const details = formData.get("details");
  const requestType = details.toLowerCase().includes("pedal") || details.toLowerCase().includes("key")
    ? "tuning and support"
    : "tuning";

  confirmationMessage.textContent = `${name}, thanks! Your ${requestType} request preview is ready, and we’ll follow up using the contact details you entered.`;
});
