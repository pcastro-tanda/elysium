def foo
  bar do
    if re !~ :foo
      do_something
    end
  end
  puts $&
end
