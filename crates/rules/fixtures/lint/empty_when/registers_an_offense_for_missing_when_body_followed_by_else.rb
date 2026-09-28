case foo
when :bar then 1
when :baz # nothing
^^^^^^^^^ Avoid `when` branches without a body.
else 3
end
