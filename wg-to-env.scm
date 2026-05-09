#!/usr/bin/env guile
!#

;;; wg-to-env.scm — Parse a WireGuard config from stdin and merge
;;; the extracted values into .env, preserving any existing entries.
;;;
;;; Usage: cat airvpn.conf | guile wg-to-env.scm

(use-modules (ice-9 rdelim)
             (ice-9 regex)
             (srfi srfi-1)
             (srfi srfi-13))

(define (read-lines port)
  (let loop ((acc '()))
    (let ((line (read-line port)))
      (if (eof-object? line)
          (reverse acc)
          (loop (cons (string-trim-right line (lambda (c) (char=? c #\return))) acc))))))

(define (parse-kv line)
  "Return (key . value) for a WireGuard 'Key = Value' line, or #f."
  (and (not (string-prefix? "#" (string-trim line)))
       (let ((m (string-match
                 "^[[:space:]]*([A-Za-z]+)[[:space:]]*=[[:space:]]*(.*?)[[:space:]]*$"
                 line)))
         (and m (cons (match:substring m 1) (match:substring m 2))))))

(define (die msg)
  (format (current-error-port) "error: ~a\n" msg)
  (exit 1))

(define (wg-ref conf key) (or (assoc-ref conf key) ""))

(define (wg-require conf key)
  (let ((v (wg-ref conf key)))
    (when (string-null? v)
      (die (string-append "WireGuard config is missing required field: " key)))
    v))

(define (split-addresses addr)
  "Split 'ipv4/mask,ipv6/mask' into (list ipv4-cidr ipv6-cidr)."
  (let* ((parts (filter (lambda (s) (not (string-null? (string-trim s))))
                        (string-split addr #\,)))
         (v4 (or (find (lambda (p) (not (string-contains (string-trim p) ":"))) parts) ""))
         (v6 (or (find (lambda (p)      (string-contains (string-trim p) ":"))  parts) "")))
    (list (string-trim v4) (string-trim v6))))

(define (split-endpoint ep)
  "Split 'host:port' into (list host port)."
  (let ((m (string-match "^(.+):([0-9]+)$" (string-trim ep))))
    (if m
        (list (match:substring m 1) (match:substring m 2))
        (list (string-trim ep) ""))))

(define (read-env path)
  "Read KEY=VALUE file into an alist."
  (if (file-exists? path)
      (filter-map
       (lambda (line)
         (let ((m (string-match "^([A-Z_][A-Z0-9_]*)=(.*)$" line)))
           (and m (cons (match:substring m 1) (match:substring m 2)))))
       (call-with-input-file path read-lines))
      '()))

(define (env-merge base updates)
  "Prepend updates to base, removing any shadowed keys from base."
  (let ((update-keys (map car updates)))
    (append updates
            (filter (lambda (kv) (not (member (car kv) update-keys string=?)))
                    base))))

(define (write-env path env)
  (call-with-output-file path
    (lambda (port)
      (for-each (lambda (kv) (format port "~a=~a\n" (car kv) (cdr kv)))
                env))))

(define (main)
  (let* ((conf    (filter-map parse-kv (read-lines (current-input-port))))
         (addrs   (split-addresses (wg-require conf "Address")))
         (ep      (split-endpoint  (wg-require conf "Endpoint")))
         (tunnel-ipv6 (cadr addrs))
         (ep-ip       (car ep))
         (ep-port     (cadr ep)))
    (when (string-null? tunnel-ipv6)
      (die "Address field contains no IPv6 address"))
    (when (string-null? ep-port)
      (die (string-append "Endpoint does not contain a port: " (wg-ref conf "Endpoint"))))
    (let* ((env     (read-env ".env"))
           (updates `(("AIRVPN_PRIVATE_KEY"   . ,(wg-require conf "PrivateKey"))
                      ("AIRVPN_TUNNEL_IPV4"   . ,(car addrs))
                      ("AIRVPN_TUNNEL_IPV6"   . ,tunnel-ipv6)
                      ("AIRVPN_MTU"           . ,(wg-ref conf "MTU"))
                      ("AIRVPN_SERVER_PUBKEY" . ,(wg-require conf "PublicKey"))
                      ("AIRVPN_PRESHARED_KEY" . ,(wg-require conf "PresharedKey"))
                      ("AIRVPN_ENDPOINT_IP"   . ,ep-ip)
                      ("AIRVPN_ENDPOINT_PORT" . ,ep-port)
                      ("AIRVPN_KEEPALIVE"     . ,(wg-ref conf "PersistentKeepalive")))))
      (write-env ".env" (env-merge env updates))
      (format (current-error-port) ".env updated.\n"))))

(main)
