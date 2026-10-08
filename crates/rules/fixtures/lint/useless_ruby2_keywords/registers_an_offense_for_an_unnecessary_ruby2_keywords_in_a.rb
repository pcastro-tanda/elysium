def foo(**kwargs)
end
ruby2_keywords :foo if respond_to?(:ruby2_keywords, true)
^^^^^^^^^^^^^^^^^^^ `ruby2_keywords` is unnecessary for method `foo`.
