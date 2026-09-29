case cop_config['EnforcedStyle']
when 'single_quotes' then true
when 'double_quotes' then false
else raise 'Unknown StringLiterals style'
     ^^^^^ Use `fail` instead of `raise` to signal exceptions.
end
