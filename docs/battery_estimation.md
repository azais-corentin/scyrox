The current battery estimation algorithm uses a simple look up table to map battery voltage to state of charge.

This works well for the 0-10% and 90-100% ranges, but fails to estimate the SoC correctly for the rest of the time.
This is because the voltage to SoC curve is very flat in the 10-90% region:
![Typical Li-Ion discharge voltage curve](https://siliconlightworks.com/image/data/Info_Pages/Li-ion%20Discharge%20Voltage%20Curve%20Typical.jpg)

There are many advanced methods to estimate the SoC without direct current measurements, but we can get away with a much simpler algorithm with two assumptions:

- The mouse doesn't drain the battery while sleeping.
- When awake, the current draw is roughly constant.

This means that we can adjust our estimate of the battery level by measuring the time spent awake.

The current draw depends on the current mouse configuration. The following settings affect the current draw:

- Polling rate
- Long range mode
- Competition mode
- Lighting mode
- Performance mode
- 20K fps sensor mode

The look up table has low accuracy in some parts, but no drift.
The awake time method has high accuracy, but drifts over time.

The look up table and awake time method can be combined to produce a better estimate of the battery level.
