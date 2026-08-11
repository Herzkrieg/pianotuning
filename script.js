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
  const repair = formData.get("repair") === "on";
  const priority = formData.get("priority") === "on";
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
plannerForm.addEventListener("change", updatePlanner);
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
  const email = formData.get("email");
  const details = formData.get("details");

  confirmationMessage.textContent = `${name}, thanks! We’d prepare a local support reply for ${email} that says: “Hello, I’m looking for piano tuning support. ${details}”`;
});
