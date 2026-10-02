case foo
in [a] then 1
in [a, b] # nothing
^^^^^^^^^ Avoid `in` branches without a body.
else 3
end
