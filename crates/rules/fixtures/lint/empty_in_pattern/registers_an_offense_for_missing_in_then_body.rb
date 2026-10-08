case foo
in [a] then 1
in [a, b] then # nothing
^^^^^^^^^ Avoid `in` branches without a body.
end
