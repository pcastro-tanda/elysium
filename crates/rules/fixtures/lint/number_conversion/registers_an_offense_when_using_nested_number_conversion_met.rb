var.to_i.to_f
^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `var.to_i`, use stricter `Integer(var, 10)`.
