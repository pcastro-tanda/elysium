it 'does something' do
  $n = do_something
  value($n[:foo]).must_be 42
end
