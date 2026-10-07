it 'does something' do
  $n = do_something
  $n.path_wont_exist 42
  ^^ Use `expect($n)` instead.
end
