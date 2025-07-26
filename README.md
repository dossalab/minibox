## Minibox

This is a small Bluetooth receiver for remote controlled toys or small models. Receiver has 5 hobby servo / ESC outputs.
Since Xbox One's controller is essentially a Bluetooth gamepad, it is possible to bridge two together, turning them into a complete remote control solution.

### Connection

Switch your receiver and gamepad on. The LED on the receiver should start blinking quickly, indicating that it is in scanning mode. Once it will discover the controller it will start connecting automatically. If the connection is successful LED will turn off - then you're ready to go!

### Some considerations

- Range & reliability is probably not that great. It should be alright for cars and other ground vehicles but flying a drone like that could be dangerous (although I did);
- In case of inactivity (user is not interacting with the controller / not moving sticks), servos have to be reset to their initial position. That's because Xbox controller is designed for gaming, not for remote control. Once you stop moving sticks, controller enters low-power mode and stops sending data. Same happens when your model gets out of reach. It is hard for the receiver to distinguish these scenarios, so it has to disarm outputs in both cases (otherwise your model is at risk of running away :)

