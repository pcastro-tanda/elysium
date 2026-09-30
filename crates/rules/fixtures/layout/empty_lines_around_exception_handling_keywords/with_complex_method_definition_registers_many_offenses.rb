def foo

  do_something1

^{} Extra empty line detected before the `rescue`.
rescue RuntimeError

^{} Extra empty line detected after the `rescue`.
  do_something2

^{} Extra empty line detected before the `rescue`.
rescue ArgumentError => ex

^{} Extra empty line detected after the `rescue`.
  do_something3

^{} Extra empty line detected before the `rescue`.
rescue

^{} Extra empty line detected after the `rescue`.
  do_something3

^{} Extra empty line detected before the `else`.
else

^{} Extra empty line detected after the `else`.
  do_something4

^{} Extra empty line detected before the `ensure`.
ensure

^{} Extra empty line detected after the `ensure`.
  do_something4

end
