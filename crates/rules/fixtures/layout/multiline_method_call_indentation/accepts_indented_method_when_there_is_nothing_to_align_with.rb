expect { custom_formatter_class('NonExistentClass') }
  .to raise_error(NameError)
