args = [1,2,3]
args[0].to_i
^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `args[0].to_i`, use stricter `Integer(args[0], 10)`.
