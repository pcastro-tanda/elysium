it 'does something' do
  $n = do_something
  $n.wont_match 42
  ^^ Use `value($n)` instead.
end
