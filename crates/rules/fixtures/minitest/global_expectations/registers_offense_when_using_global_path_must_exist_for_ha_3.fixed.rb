it 'does something' do
  n = do_something
  _(n[:foo]).path_must_exist 42
end
