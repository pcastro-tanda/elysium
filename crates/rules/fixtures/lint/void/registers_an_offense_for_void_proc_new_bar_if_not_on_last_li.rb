def foo
  Proc.new { bar }
  ^^^^^^^^^^^^^^^^ `Proc.new { bar }` used in void context.
  top
end
