it 'does something' do
  $n = do_something
  value($n[:foo]).must_include 42
end
