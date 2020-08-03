/*
 *
 * . 
 *  -> NS = []
 * ca 
 *  -> NS = [c.ca-servers.ca, j.ca-servers.ca, x.ca-servers.ca, any.ca-servers.ca]
 *
 * any.ca-servers.ca
 *  -> A = []
 *  -> AAAA =[]
 *  
 * x.ca-servers.ca
 *  -> A = [] 
 *  -> AAAA = []
 *
 * j.ca-servers.net
 *  -> A = []
 *  -> AAAA = []
 *
 * x.ca-servers.net
 *  -> A = []
 *  -> AAAA = []
 *
 * Question - What is the A of shopify.ca?
 *
 * 1. Does shopify.ca/AAAA exist in cache? - No.
 * 2. Does shopify.ca/NS exist in cache? - No.
 * 3. How can we get name servers for shopify.ca? Should we ask name servers for "ca"? - Yes.
 * 4. Name servers for "ca" owner are c/j/x/any.ca-servers.ca.  Cool,  Do we have A or AAAA records
 *    for these name servers? - Yes.
 * 5. Ask c.ca-servers.ca/A for name servers for "shopify.ca".
 *
 * shopify.ca
 *  -> NS
 *      1. ns1.dnsimple.com
 *      2. ns2.dnsimple.com
 *      3. ns3.dnsimple.com
 *      4. ns4.dnsimple.com
 *      5. dns1.p06.nsone.net
 *      5. dns2.p06.nsone.net
 *      5. dns3.p06.nsone.net
 *      5. dns4.p06.nsone.net
 *
 *  Cool,  thats good progress.  But where are the glue records?  A and AAAA records are not
 *  returned by "ca"'s name server does not know about them.  We need to query "dnsimple.com" zone
 *  to get the glue records.  
 *  So,  what do we want??? We want "ns1.dnsimple.com/A" or "ns1.dnsimple.com/AAAA" in cache. But
 *  they are not in cache.
 *  Is "dnsimple.com." present in cacne?  No.
 *  Is "com." present in cache? No.
 *  Is "." in cache? Yes.
 *  Cool,  lets use name servers for "." root to get name servers for "com" zone.
 *
 * com 
 *  -> NS = []
 * a.gtld-servers.net
 *  -> A = []
 *  -> AAAA = []
 * b.gtld-servers.net
 *  -> A = []
 *  -> AAAA = [
 * c.gtld-servers.net
 *  -> A = []
 *  -> AAAA = [
 * d.gtld-servers.net
 *  -> A = []
 *  -> AAAA = [
 * ...
 * m.gtld-servers.net
 *  -> A = []
 *  -> AAAA = []
 *
 *  Awesome,  lets use "a.gtld-servers.net" to fill cache for "dnsimple.com".
 *  
 * dnsimple.com 
 *  -> NS = []
 *
 * ns1.dnsimple.com 
 *  -> A = []
 *  -> AAAA = []
 *
 * ns2.dnsimple.com
 *  -> A = []
 *  -> AAAA = []
 *
 * ns3.dnsimple.com
 *  -> A = []
 *  -> AAAA = []
 * 
 * ns4.dnsimple.com
 *  -> A = []
 *  -> AAAA = []
 *
 * Use IP of uno of the name servers (ns1.dnsimple.com) to get A record for "shopify.ca"
 * dig @ns1.dnsimple.com shopify.ca A
 *
 * shopify.ca
 *  -> A = [35.185.82.132]
 *
 * /

