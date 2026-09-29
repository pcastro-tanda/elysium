case foo
when 1 then 2
            ^ Literal `2` used in void context.
when 3 then 4
            ^ Literal `4` used in void context.
else 5
     ^ Literal `5` used in void context.
end
puts 6
