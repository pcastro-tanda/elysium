it 'does something' do
  $n = do_something
  $n.wont_include 42
  ^^ Use `value($n)` instead.
end
