3.downto(1) do
  unless o == 1
  ^^^^^^^^^^^^^ Use `next` to skip iteration.
    puts o
  end
end
