begin

^{} Extra empty line detected at `begin` body beginning.
  do_something1
rescue RuntimeError
  do_something2
rescue ArgumentError => ex
  do_something3
rescue
  do_something3
else
  do_something4
ensure
  do_something4

^{} Extra empty line detected at `begin` body end.
end
