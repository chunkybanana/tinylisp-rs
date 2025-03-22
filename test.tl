(load library)
(comment
  token state: A list of (string tokens current-token)
  Takes a list of chars because those are easier to work with)

(def _tokens (lambda (str tokens curr-token)
  (if str
    (if (contains? (list 32 10) (h str))
      (if curr-token
        (_tokens (t str) (cons (reverse curr-token) tokens) ())
        (_tokens (t str) tokens ()))
      (if (contains? (list 40 41) (h str))
        (if curr-token
          (_tokens (t str)
            (cons (h str)
              (cons (reverse curr-token) tokens))
            ())
          (_tokens (t str) (cons (h str) tokens) ()))
        (_tokens (t str) tokens (cons (h str) curr-token))))
    (reverse
      (if curr-token
        (cons (reverse curr-token) tokens)
        tokens)))))

(def _cdepth (lambda (tokens depth acc) 
  (if tokens
    (_cdepth (t tokens)  
      (+ depth (which-paren tokens))
      (cons (+ depth (which-paren tokens)) acc))
    (reverse
      (cons
        (+ depth (which-paren tokens))
        acc)))))

(def which-paren (lambda (tokens) 
  (-
    (equal? 40 (h tokens))
    (equal? 41 (h tokens)))))

(def _complete (lambda (tokens amount) 
  (if amount
    (_complete (cons 41 tokens) (- amount 1))
    (reverse tokens))))

(def complete (lambda (tokens) 
  (_complete (reverse tokens) (last (_cdepth tokens 0 ())))))

(def expr-depth (lambda (tokens)
  (foldl max2 (cons 0 (_cdepth tokens 0 ())))))

(def pad (lambda (ls amount)
  (if amount
     (pad (cons 32 ls) (- amount 1))
     ls)))

(def _next-expr (lambda (tokens depths acc)
  (if (l 0 (h depths))
    (_next-expr (t tokens) (t depths) 
      (insert-end (h tokens) acc))
    (insert-end (h tokens) acc))))

(def next-expr (lambda (tokens)
  (_next-expr tokens (_cdepth tokens 0 ()) ())))

(def _handle-nil (lambda (tokens acc)
  (if tokens
    (if
      (*
        (equal? (h tokens) 40)
        (equal? (cadr tokens) 41))
      (_handle-nil (t (t tokens)) (cons (list 40 41) acc))
      (_handle-nil (t tokens) (cons (h tokens) acc)))
    (reverse acc))))

(def get-tokens (lambda (ls) 
  (_handle-nil
    (complete
      (_tokens ls () ())) ())))

(def wrap! (lambda (ls)
  (if (type? ls List)
    ls
    (list ls))))

(comment (get-tokens (list 97 98 99 40 97 32 98 99 40 41 41)))

(def _format (lambda (tokens str indent prev-tokens)
  (if tokens
    (if (equal? (h tokens) 41)
      (_format (t tokens) 
        (insert-end 41 str)
        (- indent 1)
        (insert-end (h tokens) prev-tokens))
      (if
        (either
          (less? (expr-depth (next-expr (concat (slice prev-tokens) tokens))) 3)
          (both
            (is-second prev-tokens)
            (less? (expr-depth (next-expr tokens)) 2)))
        (_format (t tokens) 
          (concat str
            (pad (wrap! (h tokens))
              (- 1
                (contains? (list () 40) (last prev-tokens)))))
          (+ indent (equal? (h tokens) 40))
          (insert-end (h tokens) prev-tokens))
        (_format (t tokens) 
          (concat str 
            (if (equal? 40 (last prev-tokens))
              (wrap! (h tokens))
              (cons 10 
                (pad (wrap! (h tokens)) (last (_cdepth prev-tokens 0 ()))))))
          (a indent 1)
          (insert-end (h tokens) prev-tokens))))
    str)))

(def format (lambda (str)
  (string (_format (get-tokens (chars str)) () 0 ()))))

(def slice-from (lambda (ls index)
  (if index
    (slice-from (t ls) (- index 1))
    ls)))

(def dec-if-nonzero (lambda (num) (i num (s num 1) 0)))
    
(def slice (lambda (ls)
  (slice-from ls
    (dec-if-nonzero 
      (last-index 
        (reverse
          (_cdepth (reverse ls) 0 ()))
      1)))))

(def _remove-next-expr (lambda (tokens depths)
  (if (l 0 (h depths))
    (_remove-next-expr (t tokens) (t depths))
    (t tokens))))

(def remove-next-expr (lambda (tokens)
  (_remove-next-expr tokens (_cdepth tokens 0 ()))
))

(def is-second (lambda (tokens)
  (not (remove-next-expr (t (slice tokens))))))

(comment (get-tokens (list 40 113 32 39 34 34 32 40 32 89 89)))


(format (string (list 40 41)))
(format (string (list 40 108 111 97 100 32 108 105 98 114 97 114 121)))
(format (string (list 40 113 40 49 32 50)))
(format (string (list 40 113 40 40 49 41 40 50)))
(format (string (list 40 113 32 39 34 34 34 92)))
(format (string (list 40 40 40 40 40)))
(format (string (list 40 100 32 67 40 113 40 40 81 32 86 41 40 105 32 81 40 105 40 108 32 81 32 48 41 48 40 105 32 86 40 97 40 67 40 115 32 81 40 104 32 86 41 41 86 41 40 67 32 81 40 116 32 86 41 41 41 48 41 41 49)))
(format (string (list 40 40 113 32 40 103 32 40 99 32 40 99 32 40 113 32 113 41 32 103 41 32 40 99 32 40 99 32 40 113 32 113 41 32 103 41 32 40 41 41 41 41 41 32 40 113 32 40 103 32 40 99 32 40 99 32 40 113 32 113 41 32 103 41 32 40 99 32 40 99 32 40 113 32 113 41 32 103 41 32 40 41 41 41 41 41 41)))


  (format (string (list 40 100 32 102 40 113 40 40 120 32 121 32 122 32 112 41 40 105 32 112 40 105 40 108 32 112 32 48 41 40 102 40 115 32 120 32 112 41 121 40 97 32 122 32 112 41 48 41 40 105 32 120 40 102 40 115 32 120 32 49 41 40 97 32 121 32 49 41 122 40 115 32 112 32 49 41 41 40 105 32 121 40 102 32 120 40 115 32 121 32 49 41 40 97 32 122 32 49 41 40 115 32 112 32 49 41 41 40 102 32 120 32 121 32 122 32 48 41 41 41 41 40 99 32 120 40 99 32 121 40 99 32 122 40)))

(format (string (list 40 100 101 102 32 101 118 101 110 63 32 40 108 97 109 98 100 97 32 40 110 117 109 41 32 40 100 105 118 105 100 101 115 63 32 50 32 110 117 109 41 41 41)))
(format (string (list 40 100 101 102 32 111 100 100 63 32 40 108 97 109 98 100 97 32 40 110 117 109 41 32 40 110 111 116 32 40 100 105 118 105 100 101 115 63 32 50 32 110 117 109 41 41 41 41)))


(format (string (list 40 100 101 102 32 100 105 118 105 100 101 115 63 32 40 108 97 109 98 100 97 32 40 100 105 118 105 115 111 114 32 109 117 108 116 105 112 108 101 41 32 40 105 102 32 40 110 101 103 97 116 105 118 101 63 32 100 105 118 105 115 111 114 41 32 40 100 105 118 105 100 101 115 63 32 40 110 101 103 32 100 105 118 105 115 111 114 41 32 109 117 108 116 105 112 108 101 41 32 40 105 102 32 40 110 101 103 97 116 105 118 101 63 32 109 117 108 116 105 112 108 101 41 32 40 100 105 118 105 100 101 115 63 32 100 105 118 105 115 111 114 32 40 110 101 103 32 109 117 108 116 105 112 108 101 41 41 32 40 105 102 32 40 108 101 115 115 63 32 109 117 108 116 105 112 108 101 32 100 105 118 105 115 111 114 41 32 40 122 101 114 111 63 32 109 117 108 116 105 112 108 101 41 32 40 100 105 118 105 100 101 115 63 32 100 105 118 105 115 111 114 32 40 115 117 98 50 32 109 117 108 116 105 112 108 101 32 100 105 118 105 115 111 114 41 41 41 41 41 41 41)))