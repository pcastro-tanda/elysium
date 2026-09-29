case cop_config['EnforcedStyle']
when 'single_quotes' then true
when 'double_quotes' then false
else fail 'Unknown StringLiterals style'
end
