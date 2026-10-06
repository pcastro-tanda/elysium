it 'does something' do
  $n = do_something
  $n.wont_be_within_delta 42
  ^^ Use `expect($n)` instead.
end
