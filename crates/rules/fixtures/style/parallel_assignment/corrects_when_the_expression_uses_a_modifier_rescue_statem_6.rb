def foo
  a, b = %w(1 2) rescue foo
  ^^^^^^^^^^^^^^ Do not use parallel assignment.
  something_else
end
