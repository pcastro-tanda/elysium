def foo
  bar do
    if re =~ FOO
      do_something
    end
  end
  puts $&
end
