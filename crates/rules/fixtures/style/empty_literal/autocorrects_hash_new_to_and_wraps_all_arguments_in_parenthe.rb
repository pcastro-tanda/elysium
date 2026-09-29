def foo
  super Hash.new, something
        ^^^^^^^^ Use hash literal `{}` instead of `Hash.new`.
end
